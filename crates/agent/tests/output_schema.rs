//! Structured outputs validate schemas at construction and documents at settlement.
#![allow(clippy::too_many_lines)] // Table-driven schema and document validity cases.
use brainstone_agent::{OutputError, OutputSchema};
use serde_json::json;

#[test]
fn schema_names_and_meta_schema_are_validated() {
    for name in ["", "has spaces", "é", &"a".repeat(65)] {
        assert_eq!(
            OutputSchema::new(name, json!({})).unwrap_err(),
            OutputError::InvalidName
        );
    }
    assert_eq!(
        OutputSchema::new("valid-name_1", json!({"type":7})).unwrap_err(),
        OutputError::InvalidSchema
    );
    assert_eq!(
        OutputSchema::new("valid", json!({"required":"name"})).unwrap_err(),
        OutputError::InvalidSchema
    );
}

#[test]
fn nested_values_enums_arrays_and_exact_objects_are_checked() {
    let contract = OutputSchema::new(
        "reply",
        json!({
            "type":"object", "properties":{
                "person":{"type":"object", "properties":{"name":{"type":"string"}},
                    "required":["name"], "additionalProperties":false},
                "state":{"enum":["ready","waiting"]},
                "counts":{"type":"array", "items":{"type":"integer"}, "minItems":1}
            }, "required":["person","state","counts"], "additionalProperties":false
        }),
    )
    .unwrap();
    let valid = json!({"person":{"name":"Zoë ☕"},"state":"ready","counts":[1,2]});
    assert_eq!(contract.validate(&valid.to_string()).unwrap(), valid);
    for invalid in [
        json!({"person":{},"state":"ready","counts":[1]}),
        json!({"person":{"name":"Zoë"},"state":"invalid","counts":[1]}),
        json!({"person":{"name":"Zoë"},"state":"ready","counts":[1.5]}),
        json!({"person":{"name":"Zoë"},"state":"ready","counts":[]}),
        json!({"person":{"name":"Zoë"},"state":"ready","counts":[1],"extra":true}),
    ] {
        assert_eq!(
            contract.validate(&invalid.to_string()).unwrap_err(),
            OutputError::SchemaMismatch
        );
    }
}

#[test]
fn invalid_json_and_schema_mismatch_have_the_stable_output_failure_code() {
    let contract = OutputSchema::new("reply", json!({"type":"integer"})).unwrap();
    assert_eq!(
        contract.validate("not JSON").unwrap_err(),
        OutputError::InvalidJson
    );
    assert_eq!(
        contract.validate("\"1\"").unwrap_err(),
        OutputError::SchemaMismatch
    );
    assert_eq!(OutputError::InvalidJson.code(), "structured_output_invalid");
    assert_eq!(
        OutputError::SchemaMismatch.code(),
        "structured_output_invalid"
    );
    assert_eq!(OutputError::InvalidName.code(), "invalid_output_schema");
    assert_eq!(OutputError::InvalidSchema.code(), "invalid_output_schema");
}

#[test]
fn local_definitions_resolve_but_external_references_never_do() {
    let schema = json!({"$defs":{"name":{"type":"string"}}, "$ref":"#/$defs/name"});
    assert!(
        OutputSchema::new("reply", schema)
            .unwrap()
            .validate("\"Zoë\"")
            .is_ok()
    );
    for reference in ["https://example.com/schema.json", "file:///etc/passwd"] {
        assert_eq!(
            OutputSchema::new("reply", json!({"$ref":reference})).unwrap_err(),
            OutputError::InvalidSchema
        );
    }
}

#[test]
fn serde_cannot_bypass_construction_and_valid_contracts_roundtrip() {
    let schema = json!({"type":"string", "format":"email"});
    let contract = OutputSchema::new("email", schema.clone()).unwrap();
    let document = json!({"name":"email", "schema":schema});
    assert_eq!(serde_json::to_value(&contract).unwrap(), document);
    assert_eq!(
        serde_json::from_value::<OutputSchema>(document).unwrap(),
        contract
    );
    assert_eq!(contract.name(), "email");
    assert_eq!(contract.schema(), &schema);
    assert!(contract.validate("\"a@example.com\"").is_ok());
    assert_eq!(
        contract.validate("\"not-an-email\"").unwrap_err(),
        OutputError::SchemaMismatch
    );
    assert!(
        serde_json::from_value::<OutputSchema>(json!({"name":"bad name","schema":{}})).is_err()
    );
    assert!(
        serde_json::from_value::<OutputSchema>(json!({"name":"reply","schema":{"type":7}}))
            .is_err()
    );
}

#[test]
fn stored_output_contracts_reject_unknown_future_fields() {
    let document = json!({
        "name":"answer", "schema":{"type":"object"}, "future_contract_option":true
    });
    assert!(serde_json::from_value::<OutputSchema>(document).is_err());
}
