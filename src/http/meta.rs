use super::*;

pub(super) fn configure(config: &mut web::ServiceConfig) {
    config.service(get_openapi).service(get_config);
}

#[get("/openapi.json")]
async fn get_openapi() -> impl Responder {
    HttpResponse::Ok().json(json!({
      "openapi": "3.1.0",
      "info": {"title":"Racebin API","version":"1.0.0"},
      "servers": [{"url":"/api/v1"}],
      "components": {
        "securitySchemes": {
          "bearerAuth": {"type":"http","scheme":"bearer"},
          "sessionCookie": {"type":"apiKey","in":"cookie","name":"racebin_session"}
        }
      },
      "paths": {
        "/config": {"get":{"summary":"Runtime configuration"}},
        "/session": {
          "get":{"summary":"Current session"},
          "post":{"summary":"Log in"},
          "delete":{"summary":"Log out"}
        },
        "/account/password":{"patch":{"summary":"Change current user's password"}},
        "/account/api-keys":{
          "get":{"summary":"List current user's API keys"},
          "post":{"summary":"Create an API key"}
        },
        "/account/api-keys/{id}":{
          "patch":{"summary":"Enable or disable an API key"},
          "delete":{"summary":"Delete an API key"}
        },
        "/invitations/{token}/redeem":{"post":{"summary":"Redeem an invitation"}},
        "/folders":{
          "get":{"summary":"List current user's folders"},
          "post":{"summary":"Create a folder"}
        },
        "/folders/{folder_id}":{
          "patch":{"summary":"Rename a folder"},
          "delete":{"summary":"Delete a folder and unfile its pastes"}
        },
        "/pastes":{
          "get":{"summary":"List visible pastes"},
          "post":{"summary":"Create a paste"}
        },
        "/pastes/convert":{"post":{"summary":"Convert text and rich-text content"}},
        "/pastes/folder":{"patch":{"summary":"Move owned pastes to a folder"}},
        "/pastes/{paste_id}":{
          "get":{"summary":"Get paste metadata and content without consuming a read"},
          "patch":{"summary":"Update a paste"},
          "delete":{"summary":"Delete a paste"}
        },
        "/pastes/{paste_id}/consume":{"get":{"summary":"Read and consume a paste"}},
        "/pastes/{paste_id}/raw":{"get":{"summary":"Read raw paste content"}},
        "/pastes/{paste_id}/attachments":{"post":{"summary":"Upload paste attachments"}},
        "/pastes/{paste_id}/attachments/{attachment_id}":{
          "get":{"summary":"Download a paste attachment"},
          "delete":{"summary":"Delete a paste attachment"}
        },
        "/pastes/{paste_id}/archive":{"get":{"summary":"Download paste and attachments as ZIP"}},
        "/pastes/{paste_id}/qr":{"get":{"summary":"Generate a paste QR code"}},
        "/admin/pastes":{"get":{"summary":"List all pastes"}},
        "/admin/users":{
          "get":{"summary":"List users"}
        },
        "/admin/users/{id}":{"patch":{"summary":"Update a user"}},
        "/admin/invitations":{
          "get":{"summary":"List invitations"},
          "post":{"summary":"Create an invitation"}
        },
        "/admin/invitations/{id}":{"delete":{"summary":"Revoke an invitation"}},
        "/admin/api-keys":{"get":{"summary":"List all API keys"}},
        "/admin/api-keys/{id}":{
          "patch":{"summary":"Enable or disable any API key"},
          "delete":{"summary":"Delete any API key"}
        }
      }
    }))
}

#[get("/config")]
async fn get_config() -> impl Responder {
    HttpResponse::Ok().json(json!({
        "site_name": ARGS.site_name.as_deref().unwrap_or("Racebin"),
        "max_attachment_size_bytes": ARGS.max_attachment_size_mb * 1024 * 1024,
        "attachments_enabled": !ARGS.attachments_disabled,
        "qr_codes_enabled": ARGS.qr_codes,
        "visibility_modes": ["public", "unlisted", "private"]
    }))
}
