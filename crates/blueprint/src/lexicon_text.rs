use attricat_lexicon::UsedReference;

use crate::{AttributeDeclaration, BlueprintDefinition, BlueprintError, ViewDefinition, ViewNode};

/// Attricat text that resolves `{{…}}` lexicon references, with its location
/// for error messages. Other text, such as headings, is always literal.
pub struct LexiconText<'a> {
    pub location: String,
    pub text: &'a str,
    /// The app renders this text with a count, so it needs plural forms.
    pub counted: bool,
}

/// Every translatable label in the definition: the blueprint name and
/// description, local attribute names, descriptions and status option labels,
/// and view tab, section, column, and incoming-relationship labels.
pub fn lexicon_texts(definition: &BlueprintDefinition) -> Vec<LexiconText<'_>> {
    let mut texts = vec![LexiconText {
        location: "name".to_owned(),
        text: &definition.name,
        counted: true,
    }];
    if let Some(description) = &definition.description {
        texts.push(LexiconText {
            location: "description".to_owned(),
            text: description,
            counted: false,
        });
    }
    for attribute in &definition.attributes {
        let AttributeDeclaration::Local(attribute) = attribute else {
            continue;
        };
        if let Some(name) = &attribute.name {
            texts.push(LexiconText {
                location: format!("attribute '{}' name", attribute.code),
                text: name,
                counted: false,
            });
        }
        if let Some(description) = &attribute.description {
            texts.push(LexiconText {
                location: format!("attribute '{}' description", attribute.code),
                text: description,
                counted: false,
            });
        }
        if let Some(schema) = &attribute.value_schema {
            texts.extend(status_option_texts(
                &format!("attribute '{}'", attribute.code),
                schema,
            ));
        }
    }
    let mut views: Vec<_> = definition.views.iter().collect();
    views.sort_by_key(|(name, _)| *name);
    for (view, definition) in views {
        let location = format!("views.{view}");
        match definition {
            ViewDefinition::Table {
                columns: Some(columns),
                ..
            } => texts.extend(columns.iter().filter_map(|column| {
                column.label.as_deref().map(|text| LexiconText {
                    location: format!("{location} column '{}' label", column.field),
                    text,
                    counted: false,
                })
            })),
            ViewDefinition::Stack { children, .. }
            | ViewDefinition::Grid { children, .. }
            | ViewDefinition::Section { children, .. } => {
                node_texts(&location, children, &mut texts)
            }
            ViewDefinition::Tabs { tabs, .. } => {
                for tab in tabs {
                    tab_text(&location, "tab", &tab.label, &tab.children, &mut texts);
                }
            }
            ViewDefinition::Accordion { sections, .. } => {
                for section in sections {
                    tab_text(
                        &location,
                        "section",
                        &section.label,
                        &section.children,
                        &mut texts,
                    );
                }
            }
            ViewDefinition::Table { .. }
            | ViewDefinition::DropdownOption { .. }
            | ViewDefinition::ExtensionLayout { .. } => {}
        }
    }
    texts
}

/// Status option labels in an attribute's value schema, which resolve lexicon
/// references like attribute names. `location` names the attribute.
pub fn status_option_texts<'a>(
    location: &str,
    schema: &'a serde_json::Value,
) -> impl Iterator<Item = LexiconText<'a>> {
    attricat_validation::status::status_option_labels(schema)
        .into_iter()
        .map(move |(code, text)| LexiconText {
            location: format!("{location} status option '{code}' label"),
            text,
            counted: false,
        })
}

fn tab_text<'a>(
    location: &str,
    kind: &str,
    label: &'a str,
    children: &'a [ViewNode],
    texts: &mut Vec<LexiconText<'a>>,
) {
    texts.push(LexiconText {
        location: format!("{location} {kind} label"),
        text: label,
        counted: false,
    });
    node_texts(location, children, texts);
}

fn node_texts<'a>(location: &str, nodes: &'a [ViewNode], texts: &mut Vec<LexiconText<'a>>) {
    for node in nodes {
        match node {
            ViewNode::Stack { children, .. }
            | ViewNode::Grid { children, .. }
            | ViewNode::Section { children, .. } => node_texts(location, children, texts),
            ViewNode::Tabs { tabs, .. } => {
                for tab in tabs {
                    tab_text(location, "tab", &tab.label, &tab.children, texts);
                }
            }
            ViewNode::Accordion { sections, .. } => {
                for section in sections {
                    tab_text(
                        location,
                        "section",
                        &section.label,
                        &section.children,
                        texts,
                    );
                }
            }
            ViewNode::IncomingRelationshipList { label, .. } => texts.push(LexiconText {
                location: format!("{location} incoming_relationship_list label"),
                text: label,
                counted: false,
            }),
            ViewNode::Heading { .. }
            | ViewNode::Text { .. }
            | ViewNode::Divider { .. }
            | ViewNode::Field { .. }
            | ViewNode::RelationshipList { .. } => {}
        }
    }
}

/// Rejects malformed references in translatable labels.
pub(crate) fn validate_lexicon_references(
    definition: &BlueprintDefinition,
) -> Result<(), BlueprintError> {
    for text in lexicon_texts(definition) {
        attricat_lexicon::parse(text.text).map_err(|error| {
            BlueprintError::InvalidLexiconReference {
                location: text.location,
                error,
            }
        })?;
    }
    Ok(())
}

/// The lexicon references used by the definition's translatable labels.
/// Malformed text, which validation rejects, contributes none.
pub fn lexicon_references(definition: &BlueprintDefinition) -> Vec<UsedReference> {
    lexicon_texts(definition)
        .into_iter()
        .flat_map(|text| {
            attricat_lexicon::references(text.text)
                .unwrap_or_default()
                .into_iter()
                .map(move |reference| UsedReference {
                    reference,
                    counted: text.counted,
                })
        })
        .collect()
}
