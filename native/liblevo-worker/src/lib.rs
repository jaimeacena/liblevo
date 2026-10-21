//! Motor de Liblevo: lectura de cada formato (Xberg y PDFium), pulido, señales de duda, relectura
//! visual con OvisOCR2, lista de revisión y traducción con Hy-MT2. Los lectores y modelos corren
//! como programas aparte, atados a la app y con sus huellas comprobadas.

pub mod checkpoint;
pub mod child_process;
pub mod cleanup;
pub mod convert;
pub mod dashes;
pub mod doubt;
pub mod headings;
pub mod html_tables;
pub mod images;
pub mod pdf_images;
pub mod pdf_styles;
pub mod polish;
pub mod review;
pub mod scan_detect;
pub mod translate;
pub mod visual_reader;
pub mod xberg_reader;
