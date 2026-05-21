//! Antivirus domain interfaces re-exported from `umbrella-core`.

pub mod cleaner {
    pub use umbrella_core::antivirus::cleaner::*;
}

pub mod detector {
    pub use umbrella_core::antivirus::detector::*;
}

pub mod scanner {
    pub use umbrella_core::antivirus::scanner::*;
}

pub mod signatures {
    pub use umbrella_core::antivirus::signatures::*;
}

pub use umbrella_core::antivirus::*;
