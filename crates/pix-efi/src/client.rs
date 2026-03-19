//! Efí Pix API client implementing the `PixProvider` trait.

use chrono::{DateTime, Utc};
use reqwest::header::AUTHORIZATION;
use serde::{Deserialize, Serialize};

use pix_provider::{
    ChargeRequest, ChargeResponse, ChargeStatus, Debtor, PixCharge, PixProvider, ProviderError,
};

use crate::auth::EfiAuth;
use crate::config::EfiConfig;
use crate::EfiError;

/// Efí API client for Pix operations.
///
/// Handles authentication, request building, and response parsing
/// for the Efí (formerly Gerencianet) Pix API.
#[derive(Clone)]
pub struct EfiClient {
    auth: EfiAuth,
}

impl EfiClient {
    /// Creates a new `EfiClient` with the given configuration.
    ///
    /// Loads the mTLS certificate and prepares the HTTP client.
    ///
    /// # Errors
    ///
    /// Returns `EfiError::CertificateError` if the certificate cannot be loaded.
    pub fn new(config: EfiConfig) -> Result<Self, EfiError> {
        let auth = EfiAuth::new(config)?;
        Ok(Self { auth })
    }

    /// Creates a new `EfiClient` with a pre-built `EfiAuth` (useful for testing).
    #[cfg(test)]
    pub(crate) fn with_auth(auth: EfiAuth) -> Self {
        Self { auth }
    }

    /// Makes an authenticated GET request to the Efí API.
    async fn get(&self, path: &str) -> Result<reqwest::Response, EfiError> {
        let token = self.auth.get_token().await?;
        let url = format!("{}{path}", self.auth.base_url());

        let response = self
            .auth
            .http_client()
            .get(&url)
            .header(AUTHORIZATION, format!("Bearer {token}"))
            .send()
            .await?;

        Ok(response)
    }

    /// Makes an authenticated PUT request to the Efí API.
    async fn put<B: Serialize>(&self, path: &str, body: &B) -> Result<reqwest::Response, EfiError> {
        let token = self.auth.get_token().await?;
        let url = format!("{}{path}", self.auth.base_url());

        let response = self
            .auth
            .http_client()
            .put(&url)
            .header(AUTHORIZATION, format!("Bearer {token}"))
            .json(body)
            .send()
            .await?;

        Ok(response)
    }
}

/// Efí API request body for creating a charge.
#[derive(Debug, Serialize)]
struct EfiChargeRequest {
    calendario: EfiCalendario,
    devedor: Option<EfiDevedor>,
    valor: EfiValor,
    chave: String,
    #[serde(rename = "solicitacaoPagador")]
    solicitacao_pagador: Option<String>,
}

#[derive(Debug, Serialize)]
struct EfiCalendario {
    expiracao: u32,
}

#[derive(Debug, Serialize, Deserialize)]
struct EfiDevedor {
    nome: String,
    cpf: Option<String>,
    cnpj: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct EfiValor {
    original: String,
}

/// Efí API response for a charge.
#[derive(Debug, Deserialize)]
struct EfiChargeResponse {
    txid: String,
    status: String,
    calendario: EfiCalendarioResponse,
    valor: EfiValor,
    chave: String,
    #[serde(rename = "solicitacaoPagador")]
    solicitacao_pagador: Option<String>,
    devedor: Option<EfiDevedor>,
    #[serde(rename = "pixCopiaECola")]
    pix_copia_e_cola: Option<String>,
}

#[derive(Debug, Deserialize)]
struct EfiCalendarioResponse {
    criacao: DateTime<Utc>,
    expiracao: u32,
}

/// Efí API response for listing charges.
#[derive(Debug, Deserialize)]
struct EfiChargeListResponse {
    cobs: Vec<EfiChargeResponse>,
}

impl EfiChargeResponse {
    fn to_status(&self) -> ChargeStatus {
        match self.status.as_str() {
            "ATIVA" => ChargeStatus::Active,
            "CONCLUIDA" => ChargeStatus::Completed,
            "REMOVIDA_PELO_USUARIO_RECEBEDOR" => ChargeStatus::RemovedByUser,
            "REMOVIDA_PELO_PSP" => ChargeStatus::RemovedByPsp,
            _ => ChargeStatus::Expired,
        }
    }

    fn to_pix_charge(&self) -> PixCharge {
        let expires_at =
            self.calendario.criacao + chrono::Duration::seconds(self.calendario.expiracao as i64);

        PixCharge {
            txid: self.txid.clone(),
            status: self.to_status(),
            amount: self.valor.original.clone(),
            pix_key: self.chave.clone(),
            description: self.solicitacao_pagador.clone(),
            brcode: self.pix_copia_e_cola.clone(),
            debtor: self.devedor.as_ref().map(|d| Debtor {
                name: d.nome.clone(),
                document: d.cpf.clone().or_else(|| d.cnpj.clone()).unwrap_or_default(),
            }),
            created_at: self.calendario.criacao,
            expires_at,
        }
    }
}

impl PixProvider for EfiClient {
    async fn create_charge(&self, request: ChargeRequest) -> Result<ChargeResponse, ProviderError> {
        let txid = generate_txid();

        let debtor = request.debtor.map(|d| {
            let is_cpf = d.document.len() == 11;
            EfiDevedor {
                nome: d.name,
                cpf: if is_cpf {
                    Some(d.document.clone())
                } else {
                    None
                },
                cnpj: if !is_cpf { Some(d.document) } else { None },
            }
        });

        let body = EfiChargeRequest {
            calendario: EfiCalendario {
                expiracao: request.expiration_secs,
            },
            devedor: debtor,
            valor: EfiValor {
                original: request.amount,
            },
            chave: request.pix_key,
            solicitacao_pagador: request.description,
        };

        let path = format!("/v2/cob/{txid}");
        let response = self.put(&path, &body).await?;

        let status_code = response.status();
        if !status_code.is_success() {
            let body = response
                .text()
                .await
                .unwrap_or_else(|_| "<no body>".to_string());
            return Err(ProviderError::Http {
                status: status_code.as_u16(),
                message: body,
            });
        }

        let efi_resp: EfiChargeResponse = response.json().await.map_err(|e| {
            ProviderError::InvalidResponse(format!("failed to parse response: {e}"))
        })?;

        let status = efi_resp.to_status();
        let created_at = efi_resp.calendario.criacao;
        let expires_at =
            created_at + chrono::Duration::seconds(efi_resp.calendario.expiracao as i64);

        Ok(ChargeResponse {
            txid: efi_resp.txid,
            brcode: efi_resp.pix_copia_e_cola.unwrap_or_default(),
            status,
            created_at,
            expires_at,
        })
    }

    async fn get_charge(&self, txid: &str) -> Result<PixCharge, ProviderError> {
        let path = format!("/v2/cob/{txid}");
        let response = self.get(&path).await?;

        let status_code = response.status();
        if status_code.as_u16() == 404 {
            return Err(ProviderError::NotFound(format!("charge {txid} not found")));
        }

        if !status_code.is_success() {
            let body = response
                .text()
                .await
                .unwrap_or_else(|_| "<no body>".to_string());
            return Err(ProviderError::Http {
                status: status_code.as_u16(),
                message: body,
            });
        }

        let efi_resp: EfiChargeResponse = response.json().await.map_err(|e| {
            ProviderError::InvalidResponse(format!("failed to parse response: {e}"))
        })?;

        Ok(efi_resp.to_pix_charge())
    }

    async fn list_charges(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<PixCharge>, ProviderError> {
        let path = format!(
            "/v2/cob?inicio={}&fim={}",
            start.to_rfc3339(),
            end.to_rfc3339()
        );

        let response = self.get(&path).await?;

        let status_code = response.status();
        if !status_code.is_success() {
            let body = response
                .text()
                .await
                .unwrap_or_else(|_| "<no body>".to_string());
            return Err(ProviderError::Http {
                status: status_code.as_u16(),
                message: body,
            });
        }

        let efi_resp: EfiChargeListResponse = response.json().await.map_err(|e| {
            ProviderError::InvalidResponse(format!("failed to parse response: {e}"))
        })?;

        Ok(efi_resp.cobs.iter().map(|c| c.to_pix_charge()).collect())
    }

    fn provider_name(&self) -> &str {
        "efi"
    }
}

/// Generates a random transaction ID (26 alphanumeric characters).
///
/// The Efí API requires txid to be alphanumeric ([a-zA-Z0-9]) and between
/// 26 and 35 characters. We generate a UUID v4 without hyphens, prefixed
/// with "pix" for easy identification.
fn generate_txid() -> String {
    let uuid = uuid::Uuid::new_v4().simple().to_string();
    format!("pix{uuid}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_txid() {
        let txid = generate_txid();
        assert!(!txid.is_empty());
        assert!(txid.starts_with("pix"));
    }

    #[test]
    fn test_efi_status_mapping() {
        let resp = EfiChargeResponse {
            txid: "test".to_string(),
            status: "ATIVA".to_string(),
            calendario: EfiCalendarioResponse {
                criacao: Utc::now(),
                expiracao: 3600,
            },
            valor: EfiValor {
                original: "10.00".to_string(),
            },
            chave: "key".to_string(),
            solicitacao_pagador: None,
            devedor: None,
            pix_copia_e_cola: None,
        };
        assert_eq!(resp.to_status(), ChargeStatus::Active);
    }

    #[test]
    fn test_efi_status_completed() {
        let resp = EfiChargeResponse {
            txid: "test".to_string(),
            status: "CONCLUIDA".to_string(),
            calendario: EfiCalendarioResponse {
                criacao: Utc::now(),
                expiracao: 3600,
            },
            valor: EfiValor {
                original: "10.00".to_string(),
            },
            chave: "key".to_string(),
            solicitacao_pagador: None,
            devedor: None,
            pix_copia_e_cola: None,
        };
        assert_eq!(resp.to_status(), ChargeStatus::Completed);
    }

    #[test]
    fn test_efi_to_pix_charge() {
        let now = Utc::now();
        let resp = EfiChargeResponse {
            txid: "txid123".to_string(),
            status: "ATIVA".to_string(),
            calendario: EfiCalendarioResponse {
                criacao: now,
                expiracao: 3600,
            },
            valor: EfiValor {
                original: "50.00".to_string(),
            },
            chave: "user@example.com".to_string(),
            solicitacao_pagador: Some("Test payment".to_string()),
            devedor: Some(EfiDevedor {
                nome: "João".to_string(),
                cpf: Some("52998224725".to_string()),
                cnpj: None,
            }),
            pix_copia_e_cola: Some("brcode_payload".to_string()),
        };

        let charge = resp.to_pix_charge();
        assert_eq!(charge.txid, "txid123");
        assert_eq!(charge.amount, "50.00");
        assert_eq!(charge.pix_key, "user@example.com");
        assert_eq!(charge.description, Some("Test payment".to_string()));
        assert_eq!(charge.brcode, Some("brcode_payload".to_string()));
        assert!(charge.debtor.is_some());
    }

    #[test]
    fn test_provider_name() {
        let config = EfiConfig {
            client_id: "id".to_string(),
            client_secret: "secret".to_string(),
            certificate_path: "/nonexistent/cert.p12".into(),
            certificate_password: String::new(),
            environment: crate::config::EfiEnvironment::Sandbox,
        };
        let auth = EfiAuth::with_client(config, reqwest::Client::new());
        let client = EfiClient::with_auth(auth);
        assert_eq!(client.provider_name(), "efi");
    }
}
