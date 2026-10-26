//! # Visualizador de topología GPU/PCIe (proyecto 85)
//!
//! - [`modelo`]: BDF, clases, enlaces PCIe (generación y GB/s).
//! - [`sysfs`]: lectura de `/sys/bus/pci/devices` + generador de sysfs simulado.
//! - [`topologia`]: árbol, ancestros, matriz de afinidad GPU×GPU, cuellos de botella.
//! - [`ui`]: TUI con ratatui (testeable con `TestBackend`).
//!
//! Sólo **lee** sysfs: no requiere root ni toca el hardware.

pub mod modelo;
pub mod sysfs;
pub mod topologia;
pub mod ui;
