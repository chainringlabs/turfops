//! Stored connections to Home Assistant and OpenWeatherMap. A secret is written and
//! used, and never returned by an endpoint or printed.

use crate::error::Result;
use sqlx::PgPool;

/// A service a connection is stored for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Service {
    HomeAssistant,
    OpenWeatherMap,
}

impl Service {
    pub(super) fn name(self) -> &'static str {
        match self {
            Service::HomeAssistant => "HomeAssistant",
            Service::OpenWeatherMap => "OpenWeatherMap",
        }
    }
}

/// A stored connection. `Debug` never prints the secret.
#[derive(Clone, PartialEq)]
pub struct Credential {
    pub address: Option<String>,
    pub secret: String,
}

impl std::fmt::Debug for Credential {
    // @spec SETUP-CRED-001, WX-SRC-ERR-002
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Credential")
            .field("address", &self.address)
            .field("secret", &"<redacted>")
            .finish()
    }
}

/// The connection stored for a service, when one is set.
pub async fn credential(pool: &PgPool, service: Service) -> Result<Option<Credential>> {
    let row: Option<(Option<String>, Option<String>)> =
        sqlx::query_as("SELECT address, secret FROM service_credential WHERE service = $1")
            .bind(service.name())
            .fetch_optional(pool)
            .await?;
    Ok(row.and_then(|(address, secret)| {
        Some(Credential {
            address,
            secret: secret?,
        })
    }))
}

/// Whether a connection for the service was ever stored, even one since forgotten.
// @spec SETUP-CRED-002
pub async fn credential_ever_stored(pool: &PgPool, service: Service) -> Result<bool> {
    Ok(
        sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM service_credential WHERE service = $1)")
            .bind(service.name())
            .fetch_one(pool)
            .await?,
    )
}

pub async fn store_credential(pool: &PgPool, service: Service, held: &Credential) -> Result<()> {
    sqlx::query(
        "INSERT INTO service_credential (service, address, secret) VALUES ($1, $2, $3) \
         ON CONFLICT (service) DO UPDATE SET \
             address = EXCLUDED.address, secret = EXCLUDED.secret, set_at = NOW()",
    )
    .bind(service.name())
    .bind(&held.address)
    .bind(&held.secret)
    .execute(pool)
    .await?;
    Ok(())
}

/// Deletes a service's stored address and secret. The emptied row stays, so the
/// connection is not imported from the environment again.
// @spec SETUP-CRED-003
pub async fn forget_credential(pool: &PgPool, service: Service) -> Result<()> {
    sqlx::query(
        "UPDATE service_credential SET address = NULL, secret = NULL, set_at = NOW() \
         WHERE service = $1",
    )
    .bind(service.name())
    .execute(pool)
    .await?;
    Ok(())
}
