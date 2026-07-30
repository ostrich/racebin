use super::*;

pub(super) fn configure(config: &mut web::ServiceConfig) {
    config.service(get_openapi).service(get_config);
}

#[get("/openapi.json")]
async fn get_openapi() -> impl Responder {
    HttpResponse::Ok().json(json!({
      "openapi": "3.1.0",
      "info": {"title":"Racebin API","version":"3.0.0"},
      "servers": [{"url":"/api/v2"}],
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
        "/invites/{token}/accept":{"post":{"summary":"Accept an invitation"}},
        "/pastes":{
          "get":{"summary":"List visible pastes"},
          "post":{"summary":"Create a paste"}
        },
        "/pastes/{slug}":{
          "get":{"summary":"Get paste metadata and content without consuming a read"},
          "patch":{"summary":"Update a paste"},
          "delete":{"summary":"Delete a paste"}
        },
        "/pastes/{slug}/consume":{"get":{"summary":"Read and consume a paste"}},
        "/pastes/{slug}/raw":{"get":{"summary":"Read raw paste content"}},
        "/pastes/{slug}/files":{"post":{"summary":"Upload paste files"}},
        "/pastes/{slug}/files/{file_id}":{
          "get":{"summary":"Download a paste file"},
          "delete":{"summary":"Delete a paste file"}
        },
        "/pastes/{slug}/archive":{"get":{"summary":"Download paste and files as ZIP"}},
        "/pastes/{slug}/qr":{"get":{"summary":"Generate a paste QR code"}},
        "/admin/pastes":{"get":{"summary":"List all pastes"}},
        "/admin/users":{
          "get":{"summary":"List users"}
        },
        "/admin/users/{id}":{"patch":{"summary":"Update a user"}},
        "/admin/invites":{
          "get":{"summary":"List invitations"},
          "post":{"summary":"Create an invitation"}
        },
        "/admin/invites/{id}":{"delete":{"summary":"Revoke an invitation"}},
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
        "name": ARGS.title.as_deref().unwrap_or("Racebin"),
        "max_file_size": ARGS.max_file_size_mb * 1024 * 1024,
        "file_uploads": !ARGS.no_file_upload,
        "qr": ARGS.qr,
        "access_modes": ["public", "unlisted", "owner"]
    }))
}
