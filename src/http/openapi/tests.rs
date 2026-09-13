use super::*;

#[test]
fn generated_contract_contains_every_supported_route() {
    let document = ApiDoc::openapi();
    let paths = document.paths.paths;
    for path in [
        "/",
        "/openapi.json",
        "/pastes",
        "/pastes/{paste_id}",
        "/pastes/{paste_id}/raw",
        "/pastes/{paste_id}/source",
        "/pastes/{paste_id}/reads",
        "/content-conversions",
        "/capabilities",
        "/languages",
        "/health",
        "/readiness",
        "/session",
        "/account/password",
        "/password-resets/{token}",
        "/invitations/{token}/redeem",
        "/account/api-keys",
        "/account/api-keys/{id}",
        "/folders",
        "/folders/{folder_id}",
        "/pastes/{paste_id}/attachments",
        "/pastes/{paste_id}/attachments/{attachment_id}",
        "/pastes/{paste_id}/archive",
        "/pastes/{paste_id}/qr",
        "/admin/users",
        "/admin/summary",
        "/admin/users/{id}",
        "/admin/users/{id}/password-reset",
        "/admin/users/{id}/sessions",
        "/admin/users/{id}/api-keys",
        "/admin/pastes",
        "/admin/invitations",
        "/admin/invitations/{id}",
        "/admin/api-keys",
        "/admin/api-keys/{id}",
    ] {
        assert!(paths.contains_key(path), "missing OpenAPI path {path}");
    }
    assert!(!paths.contains_key("/pastes/{paste_id}/consume"));
    let actual_operation_ids = paths
        .values()
        .flat_map(|item| {
            [
                item.get.as_ref(),
                item.post.as_ref(),
                item.patch.as_ref(),
                item.delete.as_ref(),
                item.put.as_ref(),
            ]
            .into_iter()
            .flatten()
            .filter_map(|operation| operation.operation_id.as_deref())
        })
        .collect::<std::collections::HashSet<_>>();
    let expected_operation_ids = [
        "list_pastes",
        "create_paste",
        "get_paste",
        "get_paste_raw",
        "get_paste_source",
        "read_paste",
        "update_paste",
        "delete_paste",
        "convert_paste_content",
        "upload_attachments",
        "get_attachment",
        "delete_attachment",
        "get_archive",
        "get_qr",
        "list_folders",
        "create_folder",
        "rename_folder",
        "delete_folder",
        "move_pastes",
        "get_session",
        "login",
        "logout",
        "reauthenticate",
        "change_password",
        "reset_password",
        "redeem_invitation",
        "list_keys",
        "create_key",
        "update_key",
        "delete_key",
        "admin_users",
        "admin_summary",
        "admin_user",
        "admin_pastes",
        "admin_update_user",
        "admin_update_user_role",
        "admin_transfer_ownership",
        "admin_settings",
        "admin_replace_settings",
        "admin_audit_events",
        "admin_create_password_reset",
        "admin_revoke_user_sessions",
        "admin_revoke_user_keys",
        "admin_invitations",
        "admin_create_invitation",
        "admin_update_invitation",
        "admin_revoke_invitation",
        "admin_keys",
        "admin_update_key",
        "admin_delete_key",
        "api_root",
        "get_openapi",
        "get_capabilities",
        "get_languages",
        "get_health",
        "get_readiness",
    ]
    .into_iter()
    .collect::<std::collections::HashSet<_>>();
    assert_eq!(actual_operation_ids, expected_operation_ids);
    for (method, path) in [
        ("get", "/session"),
        ("post", "/session"),
        ("delete", "/session"),
        ("get", "/folders"),
        ("post", "/folders"),
        ("patch", "/folders/{folder_id}"),
        ("delete", "/folders/{folder_id}"),
        ("get", "/account/api-keys"),
        ("post", "/account/api-keys"),
        ("patch", "/account/api-keys/{id}"),
        ("delete", "/account/api-keys/{id}"),
        ("post", "/pastes/{paste_id}/attachments"),
        ("get", "/pastes/{paste_id}/attachments/{attachment_id}"),
        ("delete", "/pastes/{paste_id}/attachments/{attachment_id}"),
        ("get", "/admin/users"),
        ("patch", "/admin/users/{id}"),
        ("get", "/admin/invitations"),
        ("post", "/admin/invitations"),
        ("get", "/health"),
        ("get", "/readiness"),
    ] {
        let item = paths.get(path).unwrap();
        let present = match method {
            "get" => item.get.is_some(),
            "post" => item.post.is_some(),
            "patch" => item.patch.is_some(),
            "delete" => item.delete.is_some(),
            _ => false,
        };
        assert!(present, "missing OpenAPI operation {method} {path}");
    }
}

#[test]
fn every_operation_has_responses_and_resolvable_schema_references() {
    let value = serde_json::to_value(ApiDoc::openapi()).unwrap();
    let paths = value["paths"].as_object().unwrap();
    let mut operation_ids = std::collections::HashSet::new();
    for (path, item) in paths {
        for method in ["get", "post", "patch", "delete", "put"] {
            let Some(operation) = item.get(method) else {
                continue;
            };
            let responses = operation["responses"]
                .as_object()
                .unwrap_or_else(|| panic!("{method} {path} has no response object"));
            assert!(!responses.is_empty(), "{method} {path} has no responses");
            let operation_id = operation["operationId"]
                .as_str()
                .unwrap_or_else(|| panic!("{method} {path} has no operationId"));
            assert!(
                operation_ids.insert(operation_id),
                "duplicate operationId {operation_id}"
            );
            for (status, response) in responses {
                let Some(content) = response["content"].as_object() else {
                    continue;
                };
                let has_problem_schema = content.values().any(|media| {
                    media["schema"]["$ref"].as_str() == Some("#/components/schemas/ProblemDetails")
                });
                if has_problem_schema {
                    assert_eq!(
                            content.len(),
                            1,
                            "{method} {path} response {status} mixes problem details with another representation"
                        );
                    assert!(
                        content.contains_key("application/problem+json"),
                        "{method} {path} response {status} has the wrong problem media type"
                    );
                }
            }
        }
    }

    let schemas = value["components"]["schemas"].as_object().unwrap();
    fn check_refs(value: &serde_json::Value, schemas: &serde_json::Map<String, serde_json::Value>) {
        match value {
            serde_json::Value::Object(object) => {
                if let Some(reference) = object.get("$ref").and_then(|value| value.as_str()) {
                    if let Some(name) = reference.strip_prefix("#/components/schemas/") {
                        assert!(
                            schemas.contains_key(name),
                            "unresolved schema reference {reference}"
                        );
                    }
                }
                for value in object.values() {
                    check_refs(value, schemas);
                }
            }
            serde_json::Value::Array(array) => {
                for value in array {
                    check_refs(value, schemas);
                }
            }
            _ => {}
        }
    }
    check_refs(&value, schemas);
}

#[test]
fn protected_operations_and_protocol_headers_are_explicit() {
    let value = serde_json::to_value(ApiDoc::openapi()).unwrap();
    let operation =
        |method: &str, path: &str| -> &serde_json::Value { &value["paths"][path][method] };
    let anonymous_allowed = [
        "list_pastes",
        "get_paste",
        "get_paste_raw",
        "read_paste",
        "get_attachment",
        "get_archive",
        "get_qr",
        "get_session",
        "login",
        "reset_password",
        "redeem_invitation",
        "api_root",
        "get_openapi",
        "get_capabilities",
        "get_languages",
        "get_health",
        "get_readiness",
    ]
    .into_iter()
    .collect::<std::collections::HashSet<_>>();
    for (path, item) in value["paths"].as_object().unwrap() {
        for method in ["get", "post", "patch", "delete", "put"] {
            let Some(operation) = item.get(method) else {
                continue;
            };
            let operation_id = operation["operationId"].as_str().unwrap();
            let security = operation["security"]
                .as_array()
                .unwrap_or_else(|| panic!("{method} {path} does not declare security"));
            assert!(!security.is_empty(), "{method} {path} has empty security");
            if !anonymous_allowed.contains(operation_id) {
                assert!(
                    security.iter().all(|requirement| {
                        requirement
                            .as_object()
                            .is_some_and(|requirement| !requirement.is_empty())
                    }),
                    "protected operation {method} {path} permits anonymous access"
                );
            }
        }
    }

    let parameter_names = |method: &str, path: &str| {
        operation(method, path)["parameters"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|parameter| parameter["name"].as_str())
            .collect::<std::collections::HashSet<_>>()
    };
    let list_parameters = parameter_names("get", "/pastes");
    for expected in [
        "q",
        "page",
        "page_size",
        "owner",
        "visibility",
        "format",
        "folder_id",
        "created_after",
        "min_size_bytes",
        "sort",
        "direction",
    ] {
        assert!(
            list_parameters.contains(expected),
            "missing list parameter {expected}"
        );
    }
    assert!(parameter_names("post", "/pastes").contains("Idempotency-Key"));
    assert!(parameter_names("post", "/pastes/{paste_id}/reads").contains("Idempotency-Key"));
    for (method, path) in [
        ("patch", "/pastes/{paste_id}"),
        ("delete", "/pastes/{paste_id}"),
        ("post", "/pastes/{paste_id}/attachments"),
        ("delete", "/pastes/{paste_id}/attachments/{attachment_id}"),
    ] {
        assert!(parameter_names(method, path).contains("If-Match"));
    }
    assert!(operation("post", "/pastes")["x-racebin-scopes"].is_array());
    assert!(operation("get", "/admin/users")["x-racebin-scopes"].is_array());
}

#[test]
fn every_operation_advertises_its_authoritative_api_key_scopes() {
    let value = serde_json::to_value(ApiDoc::openapi()).unwrap();
    for (path, item) in value["paths"].as_object().unwrap() {
        for method in ["get", "post", "patch", "delete", "put"] {
            let Some(operation) = item.get(method) else {
                continue;
            };
            let operation_id = operation["operationId"].as_str().unwrap();
            let expected = operation_scopes(operation_id);
            let actual = operation["x-racebin-scopes"]
                .as_array()
                .map(|scopes| {
                    scopes
                        .iter()
                        .map(|scope| scope.as_str().unwrap())
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            assert_eq!(
                actual, expected,
                "incorrect API-key scopes for {method} {path} ({operation_id})"
            );
        }
    }
}

#[test]
fn generated_client_schemas_preserve_binary_and_multipart_semantics() {
    let value = serde_json::to_value(ApiDoc::openapi()).unwrap();
    for (path, media_type) in [
        ("/pastes/{paste_id}/archive", "application/zip"),
        (
            "/pastes/{paste_id}/attachments/{attachment_id}",
            "application/octet-stream",
        ),
        ("/pastes/{paste_id}/qr", "image/png"),
    ] {
        assert_eq!(
            value["paths"][path]["get"]["responses"]["200"]["content"][media_type],
            serde_json::json!({}),
            "{path} must describe a raw binary body, not a JSON number array"
        );
    }
    let schemas = &value["components"]["schemas"];
    let upload = &schemas["AttachmentUploadRequest"];
    assert_eq!(upload["additionalProperties"], false);
    let file = &upload["properties"]["file"];
    assert_eq!(file["type"], "array");
    assert_eq!(file["minItems"], 1);
    assert_eq!(file["items"], serde_json::json!({}));
    let multipart_variants = schemas["MultipartCreateRequest"]["anyOf"]
        .as_array()
        .unwrap();
    assert_eq!(multipart_variants.len(), 6);
    let mut content_only = 0;
    let mut file_only = 0;
    for variant in multipart_variants {
        assert_eq!(variant["additionalProperties"], false);
        let file = &variant["properties"]["file"];
        assert_eq!(file["type"], "array");
        assert_eq!(file["minItems"], 1);
        assert_eq!(file["items"], serde_json::json!({}));
        let required = variant["required"].as_array().unwrap();
        let requires_content = required.iter().any(|name| name == "content");
        let requires_file = required.iter().any(|name| name == "file");
        assert_ne!(requires_content, requires_file);
        content_only += usize::from(requires_content);
        file_only += usize::from(requires_file);
    }
    assert_eq!(content_only, 3);
    assert_eq!(file_only, 3);
    let uploaded_items =
        &value["components"]["schemas"]["AttachmentUploadResponse"]["properties"]["items"];
    assert_eq!(uploaded_items["minItems"], 1);
    assert_eq!(
        uploaded_items["items"]["$ref"],
        "#/components/schemas/AttachmentUploadItem"
    );
}

#[test]
fn list_filters_are_typed_constrained_and_documented() {
    let value = serde_json::to_value(ApiDoc::openapi()).unwrap();
    let parameters = value["paths"]["/pastes"]["get"]["parameters"]
        .as_array()
        .unwrap();
    let parameter = |name: &str| {
        parameters
            .iter()
            .find(|parameter| parameter["name"] == name)
            .unwrap_or_else(|| panic!("missing list parameter {name}"))
    };
    assert_eq!(parameter("page")["schema"]["minimum"], 1);
    assert_eq!(parameter("page")["schema"]["default"], 1);
    assert_eq!(parameter("page_size")["schema"]["minimum"], 1);
    assert_eq!(
        parameter("page_size")["schema"]["maximum"],
        crate::limits::MAX_PAGE_SIZE
    );
    assert_eq!(
        parameter("page_size")["schema"]["default"],
        crate::limits::DEFAULT_PAGE_SIZE
    );
    assert_eq!(parameter("created_after")["schema"]["format"], "date-time");
    assert_eq!(parameter("created_before")["schema"]["format"], "date-time");
    for name in [
        "q",
        "owner",
        "folder_id",
        "unfiled",
        "expiration",
        "read_limit",
        "sort",
        "direction",
    ] {
        assert!(
            parameter(name)["description"]
                .as_str()
                .is_some_and(|text| !text.is_empty()),
            "list parameter {name} needs a description"
        );
    }
    assert_eq!(
        value["components"]["schemas"]["ExpirationFilter"]["enum"],
        serde_json::json!(["never", "scheduled"])
    );
    assert_eq!(
        value["components"]["schemas"]["ReadLimitFilter"]["enum"],
        serde_json::json!(["unlimited", "limited"])
    );
    assert_eq!(
        parameter("sort")["schema"]["enum"],
        serde_json::json!(["created", "title", "reads", "expires", "size"])
    );
    assert_eq!(
        parameter("direction")["schema"]["enum"],
        serde_json::json!(["asc", "desc"])
    );
    assert!(value["components"]["schemas"]["PasteSort"].is_null());
    assert!(value["components"]["schemas"]["SortDirection"].is_null());
}

#[test]
fn growing_administrative_collections_publish_page_contracts() {
    let value = serde_json::to_value(ApiDoc::openapi()).unwrap();
    for (path, schema, filters) in [
        (
            "/admin/users",
            "AdminUserPage",
            &["search", "role", "status", "sort", "direction"][..],
        ),
        (
            "/admin/api-keys",
            "ApiKeyPage",
            &["search", "status", "sort", "direction"][..],
        ),
        (
            "/account/api-keys",
            "ApiKeyPage",
            &["search", "status", "sort", "direction"][..],
        ),
        ("/admin/audit-events", "AuditEventPage", &["search"][..]),
    ] {
        let operation = &value["paths"][path]["get"];
        assert_eq!(
            operation["responses"]["200"]["content"]["application/json"]["schema"]["$ref"],
            format!("#/components/schemas/{schema}")
        );
        let parameters = operation["parameters"].as_array().unwrap();
        for name in filters.iter().chain(["page", "page_size"].iter()) {
            let parameter = parameters
                .iter()
                .find(|parameter| parameter["name"] == *name)
                .unwrap_or_else(|| panic!("{path} is missing {name}"));
            assert!(
                parameter["description"]
                    .as_str()
                    .is_some_and(|description| !description.is_empty()),
                "{path} parameter {name} is undocumented"
            );
        }
    }
}

#[test]
fn creation_parameters_express_time_and_collection_constraints() {
    let value = serde_json::to_value(ApiDoc::openapi()).unwrap();
    let parameters = value["paths"]["/pastes"]["post"]["parameters"]
        .as_array()
        .unwrap();
    let parameter = |name: &str| {
        parameters
            .iter()
            .find(|parameter| parameter["name"] == name)
            .unwrap_or_else(|| panic!("missing creation parameter {name}"))
    };
    assert_eq!(parameter("expires_at")["schema"]["format"], "date-time");
    assert_eq!(parameter("expires_in")["schema"]["minimum"], 1);
    assert_eq!(parameter("read_limit")["schema"]["minimum"], 1);
    assert!(value["paths"]["/pastes"]["post"]["description"]
        .as_str()
        .unwrap()
        .contains("mutually exclusive"));
    assert_eq!(
        value["components"]["schemas"]["MovePastesInput"]["properties"]["ids"]["uniqueItems"],
        true
    );
    assert_eq!(
        value["components"]["schemas"]["KeyInput"]["properties"]["scopes"]["uniqueItems"],
        true
    );
}

#[test]
fn identity_and_resource_schemas_exclude_impossible_states() {
    fn permits_null(schema: &serde_json::Value) -> bool {
        schema["type"] == "null"
            || schema["type"]
                .as_array()
                .is_some_and(|types| types.iter().any(|value| value == "null"))
            || ["oneOf", "anyOf", "allOf"].into_iter().any(|keyword| {
                schema[keyword]
                    .as_array()
                    .is_some_and(|items| items.iter().any(permits_null))
            })
    }

    let value = serde_json::to_value(ApiDoc::openapi()).unwrap();
    let schemas = &value["components"]["schemas"];
    assert_eq!(
        schemas["SessionResponse"]["oneOf"]
            .as_array()
            .unwrap()
            .len(),
        3
    );
    assert_eq!(
        schemas["BrowserSessionResponse"]["properties"]["authenticated"]["enum"],
        serde_json::json!([true])
    );
    assert_eq!(
        schemas["BearerSessionResponse"]["properties"]["authenticated"]["enum"],
        serde_json::json!([true])
    );
    assert_eq!(
        schemas["AnonymousSessionResponse"]["properties"]["authenticated"]["enum"],
        serde_json::json!([false])
    );
    assert!(schemas["PasteMetadataResource"]["properties"]["body"].is_null());
    assert!(schemas["PasteResource"]["allOf"].as_array().is_some());
    assert_eq!(schemas["UpdatePasteRequest"]["minProperties"], 1);
    assert_eq!(schemas["UserUpdate"]["minProperties"], 1);
    for schema in [
        "BrowserSessionResponse",
        "BearerSessionResponse",
        "AnonymousSessionResponse",
    ] {
        assert_eq!(schemas[schema]["additionalProperties"], false);
    }
    for property in ["title", "body", "visibility"] {
        assert!(!permits_null(
            &schemas["UpdatePasteRequest"]["properties"][property]
        ));
    }
    for property in ["enabled", "role"] {
        assert!(!permits_null(
            &schemas["UserUpdate"]["properties"][property]
        ));
    }
    assert!(schemas["ProblemDetails"]["properties"]["errors"].is_null());
    assert_eq!(
        schemas["ProblemDetails"]["properties"]["type"]["format"],
        "uri-reference"
    );
    assert_eq!(
        schemas["ProblemDetails"]["properties"]["status"]["minimum"],
        100
    );
    assert_eq!(
        schemas["ProblemDetails"]["properties"]["status"]["maximum"],
        599
    );
    assert_eq!(
        schemas["UserResource"]["properties"]["role"]["$ref"],
        "#/components/schemas/UserRole"
    );
    assert_eq!(
        schemas["InvitationResource"]["properties"]["status"]["$ref"],
        "#/components/schemas/InvitationStatus"
    );
}

#[test]
fn creation_contract_has_unambiguous_content_and_expiration_inputs() {
    let value = serde_json::to_value(ApiDoc::openapi()).unwrap();
    let operation = &value["paths"]["/pastes"]["post"];
    let parameters = operation["parameters"].as_array().unwrap();
    assert!(!parameters
        .iter()
        .any(|parameter| matches!(parameter["name"].as_str(), Some("content" | "format"))));
    for name in ["CreatePasteRequest", "FlatCreateRequest"] {
        let variants = value["components"]["schemas"][name]["oneOf"]
            .as_array()
            .unwrap();
        assert_eq!(variants.len(), 3);
        assert!(variants.iter().all(|variant| {
            !(variant["properties"]["expires_at"].is_object()
                && variant["properties"]["expires_in"].is_object())
        }));
        assert!(variants
            .iter()
            .all(|variant| variant["additionalProperties"] == false));
        assert!(variants.iter().all(|variant| {
            variant["properties"]["title"]["maxLength"] == crate::limits::MAX_TITLE_CHARACTERS
        }));
    }
    let multipart = value["components"]["schemas"]["MultipartCreateRequest"]["anyOf"]
        .as_array()
        .unwrap();
    assert_eq!(multipart.len(), 6);
    assert!(multipart.iter().all(|variant| {
        !(variant["properties"]["expires_at"].is_object()
            && variant["properties"]["expires_in"].is_object())
    }));
    assert!(multipart
        .iter()
        .all(|variant| variant["additionalProperties"] == false));
    assert!(multipart.iter().all(|variant| {
        variant["properties"]["title"]["maxLength"] == crate::limits::MAX_TITLE_CHARACTERS
    }));
    let requirement_shapes = multipart
        .iter()
        .map(|variant| {
            let required = variant["required"].as_array().unwrap();
            (
                required.iter().any(|name| name == "content"),
                required.iter().any(|name| name == "file"),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        requirement_shapes
            .iter()
            .filter(|shape| **shape == (true, false))
            .count(),
        3
    );
    assert_eq!(
        requirement_shapes
            .iter()
            .filter(|shape| **shape == (false, true))
            .count(),
        3
    );
    let description = operation["description"].as_str().unwrap();
    for phrase in [
        "text/plain creates text",
        "text/markdown creates canonical Markdown",
        "text/html imports supported markup into canonical Markdown",
        "raw request body is always the content",
        "Multipart requests require nonempty content or at least one file",
    ] {
        assert!(
            description.contains(phrase),
            "missing creation rule: {phrase}"
        );
    }
}

#[test]
fn anonymous_operations_document_invalid_bearer_responses() {
    let value = serde_json::to_value(ApiDoc::openapi()).unwrap();
    for (method, path) in [
        ("get", "/pastes/{paste_id}"),
        ("post", "/pastes/{paste_id}/reads"),
        ("get", "/pastes/{paste_id}/attachments/{attachment_id}"),
        ("get", "/pastes/{paste_id}/archive"),
        ("get", "/pastes/{paste_id}/qr"),
    ] {
        assert!(
            value["paths"][path][method]["responses"]["401"].is_object(),
            "{method} {path} does not document invalid bearer credentials"
        );
    }
}

#[test]
fn folder_mutations_return_replacement_entity_tags() {
    let value = serde_json::to_value(ApiDoc::openapi()).unwrap();
    for (method, path) in [("patch", "/pastes"), ("delete", "/folders/{folder_id}")] {
        assert_eq!(
            value["paths"][path][method]["responses"]["200"]["content"]["application/json"]
                ["schema"]["$ref"],
            "#/components/schemas/PasteRevisionResponse"
        );
    }
}

#[test]
fn optional_headers_and_authorization_alternatives_are_precise() {
    let value = serde_json::to_value(ApiDoc::openapi()).unwrap();
    for (path, item) in value["paths"].as_object().unwrap() {
        for method in ["get", "post", "patch", "delete", "put"] {
            let Some(operation) = item.get(method) else {
                continue;
            };
            for parameter in operation["parameters"].as_array().into_iter().flatten() {
                assert_ne!(parameter["name"], "Accept");
                if parameter["in"] == "header" && parameter["required"] == false {
                    let types = &parameter["schema"]["type"];
                    assert_ne!(types, &serde_json::json!(["string", "null"]), "optional header {method} {path} must be absent or a string, never JSON null");
                }
            }
        }
    }
    assert_eq!(
        value["paths"]["/pastes/{paste_id}/source"]["get"]["x-racebin-authorization"]["anyOf"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        value["paths"]["/pastes"]["post"]["responses"]["201"]["headers"]["Idempotency-Replayed"]
            ["schema"]["type"],
        "boolean"
    );
}

#[test]
fn every_timestamp_schema_uses_rfc3339_date_time_strings() {
    fn contains_date_time(schema: &serde_json::Value) -> bool {
        schema["format"].as_str() == Some("date-time")
            || schema
                .as_object()
                .into_iter()
                .flat_map(|object| object.values())
                .any(contains_date_time)
            || schema
                .as_array()
                .into_iter()
                .flatten()
                .any(contains_date_time)
    }

    let value = serde_json::to_value(ApiDoc::openapi()).unwrap();
    let schemas = value["components"]["schemas"].as_object().unwrap();
    let mut timestamps = 0;
    for (schema_name, schema) in schemas {
        let Some(properties) = schema["properties"].as_object() else {
            continue;
        };
        for (property_name, property) in properties {
            if property_name.ends_with("_at") {
                timestamps += 1;
                assert!(
                    contains_date_time(property),
                    "{schema_name}.{property_name} is not an RFC 3339 date-time string"
                );
            }
        }
    }
    assert!(timestamps > 0, "no timestamp properties were checked");
}

#[test]
fn markdown_safety_is_part_of_the_generated_contract() {
    let value = serde_json::to_value(ApiDoc::openapi()).unwrap();
    for schema_name in ["BodyInput", "BodyOutput"] {
        let schema = serde_json::to_string(&value["components"]["schemas"][schema_name])
            .unwrap()
            .to_ascii_lowercase();
        assert!(
            schema.contains("markdown"),
            "{schema_name} does not explain Markdown semantics"
        );
        if schema_name == "BodyOutput" {
            assert!(schema.contains("sanitiz") && schema.contains("html"));
        }
    }
}

#[test]
fn body_inputs_and_revision_etags_are_precise() {
    let value = serde_json::to_value(ApiDoc::openapi()).unwrap();
    let branches = value["components"]["schemas"]["BodyInput"]["oneOf"]
        .as_array()
        .unwrap();
    assert_eq!(branches.len(), 2);
    assert!(branches
        .iter()
        .all(|branch| branch["additionalProperties"] == false));
    let etag = &value["components"]["schemas"]["PasteRevisionResource"]["properties"]["etag"];
    assert!(etag["description"]
        .as_str()
        .is_some_and(|description| description.contains("verbatim in `If-Match`")));
}

#[test]
fn discovery_advertises_stable_versions_media_types_and_canonical_urls() {
    let document = serde_json::to_value(ApiDoc::openapi()).unwrap();
    let properties = document["components"]["schemas"]["Capabilities"]["properties"]
        .as_object()
        .unwrap();
    for field in [
        "server_version",
        "api_version",
        "web_base_url",
        "api_base_url",
        "paste_create_media_types",
        "attachment_upload_media_types",
    ] {
        assert!(properties.contains_key(field), "missing capability {field}");
    }
    assert_eq!(properties["max_page_size"]["minimum"], 1);

    let public_url = url::Url::parse("https://example.com/racebin/").unwrap();
    let (web, api) = canonical_base_urls(Some(&public_url));
    assert_eq!(web.as_deref(), Some("https://example.com/racebin"));
    assert_eq!(api.as_deref(), Some("https://example.com/racebin/api/v1"));
    assert_eq!(canonical_base_urls(None), (None, None));
}
