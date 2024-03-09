mod merger;

use std::error::Error;
use std::fs::File;

fn main() -> Result<(), Box<dyn Error>> {
    let paths = Vec::from([
        "tests/pdf_samples/in/pdf1.pdf",
        "tests/pdf_samples/in/pdf2.pdf",
        "tests/pdf_samples/in/pdf3.pdf",
        "tests/pdf_samples/in/pdf4.pdf",
    ]);

    match merger::merge_pdfs(&paths) {
        Ok(mut merged_document) => {
            save_document(&mut merged_document)?;
        }
        Err(e) => {
            eprintln!("Erreur lors de la fusion des documents : {}", e);
        }
    }

    println!("Done!");
    Ok(())
}

fn save_document(document: &mut lopdf::Document) -> std::io::Result<File> {
    let dynamic_path = format!(
        "tests/pdf_samples/out/merged_output_{}.pdf",
        chrono::Local::now().format("%Y-%m-%d_%H-%M-%S")
    );

    document.save(dynamic_path)
}
