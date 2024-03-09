use lopdf::{Document, Error, Object, ObjectId};
use rayon::prelude::*;
use std::collections::BTreeMap;
use std::io;

type DocumentMappings = (BTreeMap<ObjectId, Object>, BTreeMap<ObjectId, Object>);
type PdfStructureComponents = ((ObjectId, Object), (ObjectId, Object));

pub fn merge_pdfs(paths: &Vec<&str>) -> Result<Document, Error> {
    let documents = load_documents(paths)?;
    let (documents_pages, documents_objects) = process_documents(documents)?;

    merge_documents(documents_pages, documents_objects)
}

fn load_documents(paths: &Vec<&str>) -> Result<Vec<Document>, lopdf::Error> {
    paths.par_iter().map(Document::load).collect()
}

// Process each document and prepare them for merging
fn process_documents(documents: Vec<Document>) -> Result<DocumentMappings, Error> {
    let mut max_id = 1;
    let mut documents_pages = BTreeMap::new();
    let mut documents_objects = BTreeMap::new();

    for mut document in documents {
        document.renumber_objects_with(max_id);
        max_id = document.max_id + 1;

        documents_pages.extend(extract_pages(&document)?);
        documents_objects.extend(document.objects);
    }

    Ok((documents_pages, documents_objects))
}

// Extract pages from the document
fn extract_pages(document: &Document) -> Result<BTreeMap<ObjectId, Object>, Error> {
    document
        .get_pages()
        .into_values()
        .map(|object_id| Ok((object_id, document.get_object(object_id)?.to_owned())))
        .collect()
}

fn merge_documents(
    documents_pages: BTreeMap<ObjectId, Object>,
    documents_objects: BTreeMap<ObjectId, Object>,
) -> Result<Document, Error> {
    let mut document = Document::with_version("1.5");

    let ((catalog_id, catalog_object), (pages_id, pages_object)) =
        find_catalog_and_pages(&mut document, &documents_objects)?;

    insert_pages(&mut document, &documents_pages, pages_id)?;
    update_pages_object(&mut document, &pages_object, documents_pages, pages_id)?;
    update_catalog_object(&mut document, &catalog_object, catalog_id, pages_id)?;

    document.trailer.set("Root", catalog_id);
    document.max_id = document.objects.len() as u32;
    document.renumber_objects();
    document.compress();

    Ok(document)
}

fn find_catalog_and_pages(
    document: &mut Document,
    documents_objects: &BTreeMap<ObjectId, Object>,
) -> Result<PdfStructureComponents, Error> {
    let mut catalog_object: Option<(ObjectId, Object)> = None;
    let mut pages_object: Option<(ObjectId, Object)> = None;

    for (object_id, object) in documents_objects.iter() {
        match object.type_name().unwrap_or("") {
            "Catalog" => {
                if catalog_object.is_none() {
                    catalog_object = Some((*object_id, object.clone()));
                }
            }
            "Pages" => {
                if let Ok(dictionary) = object.as_dict() {
                    let mut new_dictionary = dictionary.clone();
                    if let Some((_, ref existing_object)) = pages_object {
                        if let Ok(old_dictionary) = existing_object.as_dict() {
                            new_dictionary.extend(&old_dictionary.clone());
                        }
                    }
                    pages_object = Some((*object_id, Object::Dictionary(new_dictionary)));
                }
            }
            "Page" | "Outlines" | "Outline" => {}
            _ => {
                document.objects.insert(*object_id, object.clone());
            }
        }
    }

    match (catalog_object, pages_object) {
        (Some(catalog), Some(pages)) => Ok((catalog, pages)),
        _ => Err(lopdf::Error::from(io::Error::new(
            io::ErrorKind::Other,
            "Failed to find catalog and pages objects",
        ))),
    }
}

fn insert_pages(
    document: &mut Document,
    documents_pages: &BTreeMap<ObjectId, Object>,
    pages_id: ObjectId,
) -> Result<(), Error> {
    for (object_id, object) in documents_pages.iter() {
        match object.as_dict() {
            Ok(dictionary) => {
                let mut dictionary = dictionary.clone();
                dictionary.set("Parent", pages_id);
                document
                    .objects
                    .insert(*object_id, Object::Dictionary(dictionary));
            }
            Err(e) => {
                eprintln!("Error processing page object ID {:?}: {}", object_id, e);
                return Err(e);
            }
        }
    }

    Ok(())
}

fn update_pages_object(
    document: &mut Document,
    pages_object: &Object,
    documents_pages: BTreeMap<ObjectId, Object>,
    pages_id: ObjectId,
) -> Result<(), Error> {
    if let Ok(dictionary) = pages_object.as_dict() {
        let mut dictionary = dictionary.clone();
        dictionary.set("Count", documents_pages.len() as u32);
        dictionary.set(
            "Kids",
            documents_pages
                .keys()
                .map(|&object_id| Object::Reference(object_id))
                .collect::<Vec<Object>>(),
        );
        document
            .objects
            .insert(pages_id, Object::Dictionary(dictionary));
    }
    Ok(())
}

fn update_catalog_object(
    document: &mut Document,
    catalog_object: &Object,
    catalog_id: ObjectId,
    pages_id: ObjectId,
) -> Result<(), Error> {
    if let Ok(dictionary) = catalog_object.as_dict() {
        let mut dictionary = dictionary.clone();
        dictionary.set("Pages", pages_id);
        dictionary.remove(b"Outlines");
        document
            .objects
            .insert(catalog_id, Object::Dictionary(dictionary));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use lopdf::content::{Content, Operation};
    use lopdf::{dictionary, Document, Stream};

    pub fn create_simple_pdf() -> Document {
        let mut doc = Document::with_version("1.5");
        let pages_id = doc.new_object_id();
        let font_id = doc.add_object(dictionary! {
            "Type" => "Font",
            "Subtype" => "Type1",
            "BaseFont" => "Courier",
        });
        let resources_id = doc.add_object(dictionary! {
            "Font" => dictionary! {
                "F1" => font_id,
            },
        });
        let content = Content {
            operations: vec![
                Operation::new("BT", vec![]),
                Operation::new("Tf", vec!["F1".into(), 48.into()]),
                Operation::new("Td", vec![100.into(), 600.into()]),
                Operation::new("Tj", vec![Object::string_literal("Hello World!")]),
                Operation::new("ET", vec![]),
            ],
        };
        let content_id = doc.add_object(Stream::new(dictionary! {}, content.encode().unwrap()));
        let page_id = doc.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "Contents" => content_id,
            "Resources" => resources_id,
            "MediaBox" => vec![0.into(), 0.into(), 595.into(), 842.into()],
        });
        let pages = dictionary! {
            "Type" => "Pages",
            "Kids" => vec![page_id.into()],
            "Count" => 1,
        };
        doc.objects.insert(pages_id, Object::Dictionary(pages));
        let catalog_id = doc.add_object(dictionary! {
            "Type" => "Catalog",
            "Pages" => pages_id,
        });
        doc.trailer.set("Root", catalog_id);

        doc
    }

    #[test]
    fn test_process_documents_single_doc() {
        let doc = create_simple_pdf();
        let result = process_documents(vec![doc]).unwrap();
        assert_eq!(result.0.len(), 1);
        assert!(!result.1.is_empty());
    }

    #[test]
    fn test_merge_documents() {
        let doc1 = create_simple_pdf();
        let doc2 = create_simple_pdf();
        let (documents_pages, documents_objects) = process_documents(vec![doc1, doc2]).unwrap();
        let merged_doc = merge_documents(documents_pages, documents_objects).unwrap();
        assert_eq!(merged_doc.page_iter().count(), 2); // Vérifiez le nombre de pages après fusion
    }
}
