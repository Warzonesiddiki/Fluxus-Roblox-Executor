/// # Kernel Abstraction Layer (Sandboxed Research)
///
/// This module provides a **simulated** kernel-level architecture for
/// educational research in a sandboxed/offline environment. It demonstrates
/// the structure and lifecycle of:
///
/// 1. **Driver entry / hide routine** — loading, initialising, and unlinking
///    from system module lists (DKOM concept).
/// 2. **EPT / Hypervisor stubs** — Extended Page Tables for memory
///    virtualization (Intel VT-x / AMD-V abstraction).
/// 3. **Ring 0 → Ring 3 bridge** — secure shared-memory IPC for script
///    queueing.
///
/// **All operations are simulated.** No actual kernel calls are made.
/// This is a **reference architecture** only.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use log::{info, debug};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum KernelError {
    #[error("Driver entry failed: {0}")]
    DriverEntryFailed(String),
    #[error("EPT initialisation failed: page table {0}")]
    EptInitFailed(String),
    #[error("Shadow copy not found for address 0x{0:x}")]
    ShadowCopyNotFound(u64),
    #[error("Ring 0 → Ring 3 bridge not initialised")]
    BridgeNotInitialised,
    #[error("Operation requires 'hw' feature (real kernel)")]
    HardwareOnly,
}

/// Driver load state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DriverState {
    Unloaded,
    Loaded,
    Hidden,  // DKOM — removed from loaded module list
    EptActive,
    Error,
}

/// Simulated kernel driver.
pub struct KernelDriver {
    pub state: DriverState,
    pub driver_name: String,
    hidden: AtomicBool,
    ept_tables: Mutex<Option<EptPageTables>>,
    bridge: Mutex<Option<RingBridge>>,
}

impl KernelDriver {
    pub fn new(name: &str) -> Self {
        Self {
            state: DriverState::Unloaded,
            driver_name: name.into(),
            hidden: AtomicBool::new(false),
            ept_tables: Mutex::new(None),
            bridge: Mutex::new(None),
        }
    }

    /// Simulate DriverEntry: register, allocate structures, hide.
    pub fn load(&mut self) -> Result<(), KernelError> {
        // Phase 1: DriverEntry — register with the I/O manager
        info!("[KERNEL] DriverEntry: {} loaded", self.driver_name);

        // Phase 2: Clear registry paths (simulated)
        debug!("[KERNEL] Clearing registry traces for driver");

        // Phase 3: DKOM — unlink from PsLoadedModuleList
        self.state = DriverState::Loaded;
        self.dkom_hide();
        self.state = DriverState::Hidden;

        info!("[KERNEL] Driver hidden — DKOM complete");
        Ok(())
    }

    /// Direct Kernel Object Manipulation (simulated).
    /// Removes this driver from the system's loaded module list.
    fn dkom_hide(&self) {
        // In a real driver on Windows:
        //   1. Find our own LDR_DATA_TABLE_ENTRY via the PEB
        //   2. Remove it from InLoadOrderLinks, InMemoryOrderLinks, InInitializationOrderLinks
        //   3. Clear the entry from \Registry\Machine\System\CurrentControlSet\Services\<name>
        self.hidden.store(true, Ordering::SeqCst);
        debug!("[KERNEL] DKOM hide executed for '{}'", self.driver_name);
    }

    /// Initialise Intel VT-x / AMD-V Extended Page Tables.
    pub fn ept_init(&self, guest_paddr: u64, guest_size: u64) -> Result<(), KernelError> {
        info!("[KERNEL] EPT initialisation: GPA 0x{:x} size {}", guest_paddr, guest_size);

        // In hardware mode:
        //   1. Allocate 4KB-aligned EPT PML4 table
        //   2. Map guest physical pages → host physical pages (2MB large pages)
        //   3. Set up VMCS with EPT pointer
        //   4. Invept (invalidate EPT contexts) for security

        let tables = EptPageTables {
            pml4: 0x1000,          // simulated
            pdpt: vec![0x2000u64],  // page-directory-pointer table
            page_tables: vec![vec![0x3000u64]], // page table entries
            guest_start: guest_paddr,
            guest_size,
        };

        let mut ept = self.ept_tables.lock().map_err(|_| KernelError::EptInitFailed("lock poisoned".into()))?;
        *ept = Some(tables);

        self.state = DriverState::EptActive;
        info!("[KERNEL] EPT active — shadow memory operational");
        Ok(())
    }

    /// Intercept a memory access via the EPT violation handler.
    /// Returns the shadow (modified) bytes vs the real (clean) bytes.
    pub fn ept_handle_violation(&self, guest_va: u64, original: &[u8], modified: &[u8]) -> Result<EptShadowPage, KernelError> {
        let ept = self.ept_tables.lock().map_err(|_| KernelError::EptInitFailed("lock".into()))?;
        if ept.is_none() {
            return Err(KernelError::EptInitFailed("EPT not initialised".into()));
        }

        // Simulate: when anti-cheat reads this page, it sees `original`.
        // When the mod engine reads it, it sees `modified`.
        debug!("[KERNEL] EPT violation @ 0x{:x}: clean={}b shadow={}b",
               guest_va, original.len(), modified.len());

        Ok(EptShadowPage {
            guest_address: guest_va,
            clean_bytes: original.to_vec(),
            shadow_bytes: modified.to_vec(),
        })
    }

    /// Initialise the Ring 0 → Ring 3 communication bridge.
    pub fn bridge_init(&self, shared_mem_key: &str) -> Result<(), KernelError> {
        info!("[KERNEL] Ring 0→3 bridge: shared memory '{}'", shared_mem_key);

        let bridge = RingBridge {
            shared_memory_key: shared_mem_key.into(),
            // In production: use MmMapIoSpace or allocate MDL for shared pages
            buffer_size: 0x10000, // 64 KB
            protocol_version: 2,
        };

        let mut b = self.bridge.lock().map_err(|_| KernelError::BridgeNotInitialised)?;
        *b = Some(bridge);

        info!("[KERNEL] Bridge ready — scripts can flow securely");
        Ok(())
    }

    /// Submit a script from userspace through the bridge.
    pub fn bridge_submit(&self, script: &str) -> Result<(), KernelError> {
        let b = self.bridge.lock().map_err(|_| KernelError::BridgeNotInitialised)?;
        let _bridge = b.as_ref().ok_or(KernelError::BridgeNotInitialised)?;

        // In production: write to shared memory → signal event → user-mode listener wakes
        debug!("[KERNEL] Bridge: script submitted ({} bytes)", script.len());
        Ok(())
    }
}

impl Drop for KernelDriver {
    fn drop(&mut self) {
        if self.state != DriverState::Unloaded {
            info!("[KERNEL] Driver '{}' unloading", self.driver_name);
            // In production: disable EPT, tear down VMCS, free pages, unlink
        }
    }
}

/// EPT page table hierarchy (simulated).
#[derive(Debug)]
pub struct EptPageTables {
    pml4: u64,
    pdpt: Vec<u64>,
    page_tables: Vec<Vec<u64>>,
    guest_start: u64,
    guest_size: u64,
}

/// A single shadowed page.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EptShadowPage {
    pub guest_address: u64,
    pub clean_bytes: Vec<u8>,
    pub shadow_bytes: Vec<u8>,
}

/// The Ring 0 → Ring 3 shared-memory bridge.
#[derive(Debug, Clone)]
pub struct RingBridge {
    shared_memory_key: String,
    buffer_size: usize,
    protocol_version: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test] fn test_driver_lifecycle() {
        let mut drv = KernelDriver::new("wsx_drv.sys");
        assert_eq!(drv.state, DriverState::Unloaded);
        assert!(drv.load().is_ok());
        assert_eq!(drv.state, DriverState::Hidden);
        assert!(drv.hidden.load(Ordering::SeqCst));
    }

    #[test] fn test_ept_initialization() {
        let mut drv = KernelDriver::new("wsx.sys");
        drv.load().unwrap();
        assert!(drv.ept_init(0x1000, 0x100000).is_ok());
        assert_eq!(drv.state, DriverState::EptActive);
    }

    #[test] fn test_ept_violation() {
        let drv = KernelDriver::new("wsx.sys");
        drv.load().unwrap();
        drv.ept_init(0x1000, 0x10000).unwrap();
        let result = drv.ept_handle_violation(
            0x7ffe0000,
            &[0x90, 0x90, 0x90],  // NOP slides (clean)
            &[0x48, 0x31, 0xC0],     // xor rax, rax (modified)
        ).unwrap();
        assert_eq!(result.clean_bytes, vec![0x90, 0x90, 0x90]);
        assert_eq!(result.shadow_bytes, vec![0x48, 0x31, 0xC0]);
    }

    #[test] fn test_bridge_submission() {
        let drv = KernelDriver::new("wsx.sys");
        drv.load().unwrap();
        drv.bridge_init("WSX_SHARED_MEM").unwrap();
        drv.bridge_submit("rint(hello)").unwrap();
    }

    #[test] fn test_double_hide() {
        let mut drv = KernelDriver::new("test.sys");
        drv.load().unwrap(); // calls hide
        drv.dkom_hide();     // safe to call again
        assert!(drv.hidden.load(Ordering::SeqCst));
    }
}
