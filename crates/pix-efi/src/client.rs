//! Efí Pix API client implementing the `PixProvider` trait.
//!
//! Supports immediate charges, due-date charges, Pix payments,
//! balance queries, and transaction listing.

use chrono::{DateTime, Duration, Utc};
use reqwest::header::AUTHORIZATION;
use serde::{Deserialize, Serialize};

use pix_provider::{
    Balance, ChargeRequest, ChargeResponse, ChargeStatus, Debtor, DueDateChargeRequest, PixCharge,
    PixProvider, PixTransaction, PixTransfer, ProviderError, TransactionFilter,
};

use crate::auth::EfiAuth;
use crate::config::EfiConfig;
use crate::validate;
use crate::EfiError;

/// Efí API client for Pix operations.
///
/// Handles authentication, request building, and response parsing
/// for the Efí (formerly Gerencianet) Pix API.
#[derive(Clone)]
pub struct EfiClient {
    auth: EfiAuth,
    /// Default Pix key for the account (used as sender key for outgoing payments).
    default_pix_key: Option<String>,
}

impl EfiClient {
    /// Creates a new `EfiClient` with the given configuration.
    ///
    /// # Errors
    ///
    /// Returns `EfiError::CertificateError` if the certificate cannot be loaded.
    pub fn new(config: EfiConfig) -> Result<Self, EfiError> {
        let auth = EfiAuth::new(config)?;
        Ok(Self {
            auth,
            default_pix_key: None,
        })
    }

    /// Creates a new `EfiClient` with a default Pix key for sending payments.
    pub fn with_pix_key(config: EfiConfig, pix_key: String) -> Result<Self, EfiError> {
        let auth = EfiAuth::new(config)?;
        Ok(Self {
            auth,
            default_pix_key: Some(pix_key),
        })
    }

    /// Creates a new `EfiClient` with a pre-built `EfiAuth` (useful for testing).
    #[cfg(test)]
    pub(crate) fn with_auth(auth: EfiAuth) -> Self {
        Self {
            auth,
            default_pix_key: None,
        }
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

    /// Makes an authenticated POST request to the Efí API.
    #[allow(dead_code)]
    async fn post<B: Serialize>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<reqwest::Response, EfiError> {
        let token = self.auth.get_token().await?;
        let url = format!("{}{path}", self.auth.base_url());

        let response = self
            .auth
            .http_client()
            .post(&url)
            .header(AUTHORIZATION, format!("Bearer {token}"))
            .json(body)
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

    /// Checks an HTTP response for errors and converts to `ProviderError`.
    pub fn check_response(status: reqwest::StatusCode, body: &str) -> Result<(), ProviderError> {
        if status.is_success() {
            return Ok(());
        }
        let code = status.as_u16();
        match code {
            401 => Err(ProviderError::Authentication(format!(
                "unauthorized: {body}"
            ))),
            403 => Err(ProviderError::Authentication(format!(
                "forbidden — check API scopes: {body}"
            ))),
            404 => Err(ProviderError::NotFound(body.to_string())),
            429 => Err(ProviderError::RateLimited {
                retry_after_secs: 60,
            }),
            _ => Err(ProviderError::Http {
                status: code,
                message: body.to_string(),
            }),
        }
    }

    /// Returns `true` if the given error is transient and worth retrying.
    pub fn is_retryable(err: &ProviderError) -> bool {
        matches!(
            err,
            ProviderError::RateLimited { .. }
                | ProviderError::Timeout(_)
                | ProviderError::Http { status: 503, .. }
                | ProviderError::Http { status: 502, .. }
        ) || matches!(err, ProviderError::Network(_))
    }

    /// Executes a GET request with automatic retry on transient failures.
    ///
    /// Retries up to 2 times (3 total attempts) with exponential backoff.
    async fn get_with_retry(
        &self,
        path: &str,
    ) -> Result<(reqwest::StatusCode, String), ProviderError> {
        let mut last_err = None;
        for attempt in 0u32..3 {
            if attempt > 0 {
                let delay = std::time::Duration::from_millis(500 * 2u64.pow(attempt - 1));
                tracing::debug!("retrying GET {path} after {delay:?} (attempt {attempt})");
                tokio::time::sleep(delay).await;
            }

            let response = match self.get(path).await {
                Ok(r) => r,
                Err(e) => {
                    let provider_err: ProviderError = e.into();
                    if Self::is_retryable(&provider_err) && attempt < 2 {
                        last_err = Some(provider_err);
                        continue;
                    }
                    return Err(provider_err);
                }
            };

            let status = response.status();
            let body = response
                .text()
                .await
                .unwrap_or_else(|_| "<no body>".to_string());

            if let Err(e) = Self::check_response(status, &body) {
                if Self::is_retryable(&e) && attempt < 2 {
                    last_err = Some(e);
                    continue;
                }
                return Err(e);
            }

            return Ok((status, body));
        }
        Err(last_err.unwrap_or_else(|| ProviderError::Network("max retries exceeded".to_string())))
    }

    /// Executes a PUT request with automatic retry on transient failures.
    async fn put_with_retry<B: Serialize>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<(reqwest::StatusCode, String), ProviderError> {
        let mut last_err = None;
        for attempt in 0u32..3 {
            if attempt > 0 {
                let delay = std::time::Duration::from_millis(500 * 2u64.pow(attempt - 1));
                tracing::debug!("retrying PUT {path} after {delay:?} (attempt {attempt})");
                tokio::time::sleep(delay).await;
            }

            let response = match self.put(path, body).await {
                Ok(r) => r,
                Err(e) => {
                    let provider_err: ProviderError = e.into();
                    if Self::is_retryable(&provider_err) && attempt < 2 {
                        last_err = Some(provider_err);
                        continue;
                    }
                    return Err(provider_err);
                }
            };

            let status = response.status();
            let resp_body = response
                .text()
                .await
                .unwrap_or_else(|_| "<no body>".to_string());

            if let Err(e) = Self::check_response(status, &resp_body) {
                if Self::is_retryable(&e) && attempt < 2 {
                    last_err = Some(e);
                    continue;
                }
                return Err(e);
            }

            return Ok((status, resp_body));
        }
        Err(last_err.unwrap_or_else(|| ProviderError::Network("max retries exceeded".to_string())))
    }

    /// Makes an authenticated DELETE request to the Efí API.
    async fn delete(&self, path: &str) -> Result<reqwest::Response, EfiError> {
        let token = self.auth.get_token().await?;
        let url = format!("{}{path}", self.auth.base_url());

        let response = self
            .auth
            .http_client()
            .delete(&url)
            .header(AUTHORIZATION, format!("Bearer {token}"))
            .send()
            .await?;

        Ok(response)
    }

    /// Registers a webhook URL for the given Pix key.
    ///
    /// Efí will append `/pix` to the provided URL when sending notifications.
    ///
    /// # Errors
    ///
    /// Returns `ProviderError` if the API call fails.
    pub async fn register_webhook(
        &self,
        pix_key: &str,
        webhook_url: &str,
    ) -> Result<(), ProviderError> {
        let path = format!("/v2/webhook/{pix_key}");
        let body = serde_json::json!({ "webhookUrl": webhook_url });
        let (_status, _body) = self.put_with_retry(&path, &body).await?;
        Ok(())
    }

    /// Gets the registered webhook for the given Pix key.
    ///
    /// # Errors
    ///
    /// Returns `ProviderError` if no webhook is registered or the API call fails.
    pub async fn get_webhook(&self, pix_key: &str) -> Result<WebhookInfo, ProviderError> {
        let path = format!("/v2/webhook/{pix_key}");
        let (_status, body) = self.get_with_retry(&path).await?;
        let info: WebhookInfo = serde_json::from_str(&body)
            .map_err(|e| ProviderError::InvalidResponse(format!("failed to parse webhook: {e}")))?;
        Ok(info)
    }

    /// Removes the webhook registered for the given Pix key.
    ///
    /// # Errors
    ///
    /// Returns `ProviderError` if the API call fails.
    pub async fn remove_webhook(&self, pix_key: &str) -> Result<(), ProviderError> {
        let path = format!("/v2/webhook/{pix_key}");
        let response = self.delete(&path).await.map_err(|e| {
            let pe: ProviderError = e.into();
            pe
        })?;
        let status = response.status();
        let body = response
            .text()
            .await
            .unwrap_or_else(|_| "<no body>".to_string());
        Self::check_response(status, &body)?;
        Ok(())
    }
}

/// Information about a registered webhook.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookInfo {
    /// The registered webhook URL.
    #[serde(rename = "webhookUrl")]
    pub webhook_url: String,
    /// The Pix key this webhook is registered for.
    pub chave: Option<String>,
    /// When the webhook was created.
    #[serde(rename = "criacao")]
    pub created_at: Option<String>,
}

// ── Efí API request/response types ──────────────────────────────────────────

/// Efí API request body for creating an immediate charge.
#[derive(Debug, Serialize)]
struct EfiChargeRequest {
    calendario: EfiCalendario,
    devedor: Option<EfiDevedor>,
    valor: EfiValor,
    chave: String,
    #[serde(rename = "solicitacaoPagador")]
    #[serde(skip_serializing_if = "Option::is_none")]
    solicitacao_pagador: Option<String>,
}

/// Efí API request body for creating a due-date charge (cobv).
#[derive(Debug, Serialize)]
struct EfiDueDateChargeRequest {
    calendario: EfiCalendarioCobv,
    devedor: Option<EfiDevedor>,
    valor: EfiValor,
    chave: String,
    #[serde(rename = "solicitacaoPagador")]
    #[serde(skip_serializing_if = "Option::is_none")]
    solicitacao_pagador: Option<String>,
}

#[derive(Debug, Serialize)]
struct EfiCalendario {
    expiracao: u32,
}

#[derive(Debug, Serialize)]
struct EfiCalendarioCobv {
    #[serde(rename = "dataDeVencimento")]
    data_de_vencimento: String,
    #[serde(rename = "validadeAposVencimento")]
    validade_apos_vencimento: u32,
}

#[derive(Debug, Serialize, Deserialize)]
struct EfiDevedor {
    nome: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    cpf: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
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
    pix: Option<Vec<EfiPixPayment>>,
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

/// A Pix payment within a charge response.
#[derive(Debug, Deserialize)]
struct EfiPixPayment {
    #[serde(rename = "endToEndId")]
    end_to_end_id: String,
}

/// Efí API response for a single Pix transaction.
#[derive(Debug, Deserialize)]
struct EfiPixTransaction {
    #[serde(rename = "endToEndId")]
    end_to_end_id: String,
    txid: Option<String>,
    valor: String,
    horario: String,
    #[serde(rename = "infoPagador")]
    info_pagador: Option<String>,
    pagador: Option<EfiPagador>,
}

/// Efí API response for listing Pix transactions.
#[derive(Debug, Deserialize)]
struct EfiPixListResponse {
    pix: Vec<EfiPixTransaction>,
}

#[derive(Debug, Deserialize)]
struct EfiPagador {
    cpf: Option<String>,
    cnpj: Option<String>,
    nome: Option<String>,
}

/// Efí API response for sending a Pix.
#[derive(Debug, Deserialize)]
struct EfiSendPixResponse {
    #[serde(rename = "idEnvio")]
    id_envio: String,
    #[serde(rename = "e2eId")]
    e2e_id: Option<String>,
    valor: String,
    status: String,
}

/// Efí API response for balance.
#[derive(Debug, Deserialize)]
struct EfiBalanceResponse {
    saldo: String,
}

/// Efí API request for sending a Pix.
#[derive(Debug, Serialize)]
struct EfiSendPixRequest {
    valor: String,
    pagador: EfiPagadorRequest,
    favorecido: EfiDestinatario,
}

#[derive(Debug, Serialize)]
struct EfiPagadorRequest {
    chave: String,
    #[serde(rename = "infoPagador")]
    #[serde(skip_serializing_if = "Option::is_none")]
    info_pagador: Option<String>,
}

#[derive(Debug, Serialize)]
struct EfiDestinatario {
    chave: String,
}

// ── Conversion helpers ──────────────────────────────────────────────────────

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
            self.calendario.criacao + Duration::seconds(self.calendario.expiracao as i64);
        let e2eids: Vec<String> = self
            .pix
            .as_ref()
            .map(|payments| payments.iter().map(|p| p.end_to_end_id.clone()).collect())
            .unwrap_or_default();

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
            e2eids,
        }
    }
}

// ── PixProvider implementation ──────────────────────────────────────────────

impl PixProvider for EfiClient {
    async fn create_charge(&self, request: ChargeRequest) -> Result<ChargeResponse, ProviderError> {
        let txid = request.txid.clone().unwrap_or_else(generate_txid);
        validate::validate_txid(&txid)?;
        validate::validate_amount(&request.amount)?;

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
        let (_status, response_body) = self.put_with_retry(&path, &body).await?;

        let efi_resp: EfiChargeResponse = serde_json::from_str(&response_body).map_err(|e| {
            ProviderError::InvalidResponse(format!("failed to parse response: {e}"))
        })?;

        let status = efi_resp.to_status();
        let created_at = efi_resp.calendario.criacao;
        let expires_at = created_at + Duration::seconds(efi_resp.calendario.expiracao as i64);

        Ok(ChargeResponse {
            txid: efi_resp.txid,
            brcode: efi_resp.pix_copia_e_cola.unwrap_or_default(),
            status,
            created_at,
            expires_at,
        })
    }

    async fn create_due_date_charge(
        &self,
        request: DueDateChargeRequest,
    ) -> Result<ChargeResponse, ProviderError> {
        let txid = request.txid.clone().unwrap_or_else(generate_txid);
        validate::validate_txid(&txid)?;
        validate::validate_amount(&request.amount)?;

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

        let body = EfiDueDateChargeRequest {
            calendario: EfiCalendarioCobv {
                data_de_vencimento: request.due_date,
                validade_apos_vencimento: request.days_after_due.unwrap_or(0),
            },
            devedor: debtor,
            valor: EfiValor {
                original: request.amount,
            },
            chave: request.pix_key,
            solicitacao_pagador: request.description,
        };

        let path = format!("/v2/cobv/{txid}");
        let (_status, response_body) = self.put_with_retry(&path, &body).await?;

        let efi_resp: EfiChargeResponse = serde_json::from_str(&response_body).map_err(|e| {
            ProviderError::InvalidResponse(format!("failed to parse response: {e}"))
        })?;

        let status = efi_resp.to_status();
        let created_at = efi_resp.calendario.criacao;
        let expires_at = created_at + Duration::seconds(efi_resp.calendario.expiracao as i64);

        Ok(ChargeResponse {
            txid: efi_resp.txid,
            brcode: efi_resp.pix_copia_e_cola.unwrap_or_default(),
            status,
            created_at,
            expires_at,
        })
    }

    async fn get_charge(&self, txid: &str) -> Result<PixCharge, ProviderError> {
        validate::validate_txid(txid)?;

        let path = format!("/v2/cob/{txid}");
        let (_status, response_body) = self.get_with_retry(&path).await?;

        let efi_resp: EfiChargeResponse = serde_json::from_str(&response_body).map_err(|e| {
            ProviderError::InvalidResponse(format!("failed to parse response: {e}"))
        })?;

        Ok(efi_resp.to_pix_charge())
    }

    async fn list_charges(
        &self,
        filter: TransactionFilter,
    ) -> Result<Vec<PixCharge>, ProviderError> {
        let start = filter
            .start
            .unwrap_or_else(|| Utc::now() - Duration::days(7));
        let end = filter.end.unwrap_or_else(Utc::now);

        let path = format!(
            "/v2/cob?inicio={}&fim={}",
            start.to_rfc3339(),
            end.to_rfc3339()
        );

        let (_status, response_body) = self.get_with_retry(&path).await?;

        let efi_resp: EfiChargeListResponse =
            serde_json::from_str(&response_body).map_err(|e| {
                ProviderError::InvalidResponse(format!("failed to parse response: {e}"))
            })?;

        Ok(efi_resp.cobs.iter().map(|c| c.to_pix_charge()).collect())
    }

    async fn send_pix(
        &self,
        key: &str,
        amount: &str,
        description: Option<&str>,
    ) -> Result<PixTransfer, ProviderError> {
        validate::validate_amount(amount)?;

        let sender_key = self.default_pix_key.as_deref().ok_or_else(|| {
            ProviderError::Authentication(
                "default_pix_key is required for sending Pix; set it in your profile config"
                    .to_string(),
            )
        })?;

        let id_envio = uuid::Uuid::new_v4().simple().to_string();

        let body = EfiSendPixRequest {
            valor: amount.to_string(),
            pagador: EfiPagadorRequest {
                chave: sender_key.to_string(),
                info_pagador: description.map(String::from),
            },
            favorecido: EfiDestinatario {
                chave: key.to_string(),
            },
        };

        let path = format!("/v3/gn/pix/{id_envio}");
        let (_status, response_body) = self.put_with_retry(&path, &body).await?;

        let efi_resp: EfiSendPixResponse = serde_json::from_str(&response_body).map_err(|e| {
            ProviderError::InvalidResponse(format!("failed to parse response: {e}"))
        })?;

        Ok(PixTransfer {
            e2eid: efi_resp.e2e_id.unwrap_or_default(),
            id_envio: efi_resp.id_envio,
            amount: efi_resp.valor,
            status: efi_resp.status,
            timestamp: Utc::now(),
        })
    }

    async fn get_pix(&self, e2eid: &str) -> Result<PixTransaction, ProviderError> {
        validate::validate_e2eid(e2eid)?;

        let path = format!("/v2/pix/{e2eid}");
        let (_status, response_body) = self.get_with_retry(&path).await?;

        let efi_tx: EfiPixTransaction = serde_json::from_str(&response_body).map_err(|e| {
            ProviderError::InvalidResponse(format!("failed to parse response: {e}"))
        })?;

        Ok(convert_pix_transaction(&efi_tx))
    }

    async fn list_received_pix(
        &self,
        filter: TransactionFilter,
    ) -> Result<Vec<PixTransaction>, ProviderError> {
        let start = filter
            .start
            .unwrap_or_else(|| Utc::now() - Duration::days(7));
        let end = filter.end.unwrap_or_else(Utc::now);

        let path = format!(
            "/v2/pix?inicio={}&fim={}",
            start.to_rfc3339(),
            end.to_rfc3339()
        );

        let (_status, response_body) = self.get_with_retry(&path).await?;

        let efi_resp: EfiPixListResponse = serde_json::from_str(&response_body).map_err(|e| {
            ProviderError::InvalidResponse(format!("failed to parse response: {e}"))
        })?;

        Ok(efi_resp.pix.iter().map(convert_pix_transaction).collect())
    }

    async fn get_balance(&self) -> Result<Balance, ProviderError> {
        let (_status, response_body) = self.get_with_retry("/v2/gn/saldo").await?;

        let efi_resp: EfiBalanceResponse = serde_json::from_str(&response_body).map_err(|e| {
            ProviderError::InvalidResponse(format!("failed to parse response: {e}"))
        })?;

        Ok(Balance {
            available: efi_resp.saldo,
        })
    }

    fn provider_name(&self) -> &str {
        "efi"
    }
}

/// Converts an Efí Pix transaction to the provider-level type.
fn convert_pix_transaction(efi_tx: &EfiPixTransaction) -> PixTransaction {
    let timestamp = DateTime::parse_from_rfc3339(&efi_tx.horario)
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(|e| {
            tracing::warn!(
                "failed to parse timestamp '{}': {e}, using current time",
                efi_tx.horario
            );
            Utc::now()
        });

    PixTransaction {
        e2eid: efi_tx.end_to_end_id.clone(),
        txid: efi_tx.txid.clone(),
        amount: efi_tx.valor.clone(),
        payer_name: efi_tx.pagador.as_ref().and_then(|p| p.nome.clone()),
        payer_document: efi_tx
            .pagador
            .as_ref()
            .and_then(|p| p.cpf.clone().or_else(|| p.cnpj.clone())),
        description: efi_tx.info_pagador.clone(),
        timestamp,
    }
}

/// Generates a random transaction ID (35 alphanumeric characters).
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
        assert!(
            txid.len() >= 26 && txid.len() <= 35,
            "txid length {} is out of range",
            txid.len()
        );
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
            pix: None,
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
            pix: None,
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
            pix: Some(vec![EfiPixPayment {
                end_to_end_id: "E123456".to_string(),
            }]),
        };

        let charge = resp.to_pix_charge();
        assert_eq!(charge.txid, "txid123");
        assert_eq!(charge.amount, "50.00");
        assert_eq!(charge.pix_key, "user@example.com");
        assert_eq!(charge.description, Some("Test payment".to_string()));
        assert_eq!(charge.brcode, Some("brcode_payload".to_string()));
        assert!(charge.debtor.is_some());
        assert_eq!(charge.e2eids, vec!["E123456".to_string()]);
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

    #[test]
    fn test_convert_pix_transaction() {
        let efi_tx = EfiPixTransaction {
            end_to_end_id: "E999".to_string(),
            txid: Some("tx1".to_string()),
            valor: "42.00".to_string(),
            horario: "2026-03-19T12:00:00Z".to_string(),
            info_pagador: Some("Pagamento".to_string()),
            pagador: Some(EfiPagador {
                cpf: Some("12345678900".to_string()),
                cnpj: None,
                nome: Some("Maria".to_string()),
            }),
        };

        let tx = convert_pix_transaction(&efi_tx);
        assert_eq!(tx.e2eid, "E999");
        assert_eq!(tx.txid, Some("tx1".to_string()));
        assert_eq!(tx.amount, "42.00");
        assert_eq!(tx.payer_name, Some("Maria".to_string()));
        assert_eq!(tx.payer_document, Some("12345678900".to_string()));
    }

    #[test]
    fn test_check_response_success() {
        assert!(EfiClient::check_response(reqwest::StatusCode::OK, "").is_ok());
        assert!(EfiClient::check_response(reqwest::StatusCode::CREATED, "").is_ok());
    }

    #[test]
    fn test_check_response_auth_failure() {
        let result = EfiClient::check_response(reqwest::StatusCode::UNAUTHORIZED, "bad token");
        assert!(matches!(result, Err(ProviderError::Authentication(_))));
    }

    #[test]
    fn test_check_response_not_found() {
        let result = EfiClient::check_response(reqwest::StatusCode::NOT_FOUND, "not found");
        assert!(matches!(result, Err(ProviderError::NotFound(_))));
    }

    #[test]
    fn test_check_response_rate_limited() {
        let result = EfiClient::check_response(reqwest::StatusCode::TOO_MANY_REQUESTS, "slow down");
        assert!(matches!(result, Err(ProviderError::RateLimited { .. })));
    }
}

#[cfg(test)]
mod additional_client_tests {
    use super::*;

    fn make_charge_response(status: &str) -> EfiChargeResponse {
        EfiChargeResponse {
            txid: "test_txid".to_string(),
            status: status.to_string(),
            calendario: EfiCalendarioResponse {
                criacao: Utc::now(),
                expiracao: 3600,
            },
            valor: EfiValor {
                original: "10.00".to_string(),
            },
            chave: "key@test.com".to_string(),
            solicitacao_pagador: None,
            devedor: None,
            pix_copia_e_cola: None,
            pix: None,
        }
    }

    // --- Status mapping ---

    #[test]
    fn test_status_ativa() {
        assert_eq!(
            make_charge_response("ATIVA").to_status(),
            ChargeStatus::Active
        );
    }

    #[test]
    fn test_status_concluida() {
        assert_eq!(
            make_charge_response("CONCLUIDA").to_status(),
            ChargeStatus::Completed
        );
    }

    #[test]
    fn test_status_removida_usuario() {
        assert_eq!(
            make_charge_response("REMOVIDA_PELO_USUARIO_RECEBEDOR").to_status(),
            ChargeStatus::RemovedByUser
        );
    }

    #[test]
    fn test_status_removida_psp() {
        assert_eq!(
            make_charge_response("REMOVIDA_PELO_PSP").to_status(),
            ChargeStatus::RemovedByPsp
        );
    }

    #[test]
    fn test_status_unknown_defaults_to_expired() {
        assert_eq!(
            make_charge_response("UNKNOWN").to_status(),
            ChargeStatus::Expired
        );
    }

    // --- check_response edge cases ---

    #[test]
    fn test_check_response_200() {
        assert!(EfiClient::check_response(reqwest::StatusCode::OK, "body").is_ok());
    }

    #[test]
    fn test_check_response_201() {
        assert!(EfiClient::check_response(reqwest::StatusCode::CREATED, "").is_ok());
    }

    #[test]
    fn test_check_response_204() {
        assert!(EfiClient::check_response(reqwest::StatusCode::NO_CONTENT, "").is_ok());
    }

    #[test]
    fn test_check_response_401() {
        let err = EfiClient::check_response(reqwest::StatusCode::UNAUTHORIZED, "msg").unwrap_err();
        match err {
            ProviderError::Authentication(msg) => assert!(msg.contains("msg")),
            _ => panic!("unexpected error type"),
        }
    }

    #[test]
    fn test_check_response_403() {
        let err = EfiClient::check_response(reqwest::StatusCode::FORBIDDEN, "msg").unwrap_err();
        match err {
            ProviderError::Authentication(msg) => assert!(msg.contains("msg")),
            _ => panic!("unexpected error type"),
        }
    }

    #[test]
    fn test_check_response_404() {
        let err =
            EfiClient::check_response(reqwest::StatusCode::NOT_FOUND, "not here").unwrap_err();
        match err {
            ProviderError::NotFound(msg) => assert_eq!(msg, "not here"),
            _ => panic!("unexpected error type"),
        }
    }

    #[test]
    fn test_check_response_429() {
        let err =
            EfiClient::check_response(reqwest::StatusCode::TOO_MANY_REQUESTS, "").unwrap_err();
        match err {
            ProviderError::RateLimited { retry_after_secs } => assert_eq!(retry_after_secs, 60),
            _ => panic!("unexpected error type"),
        }
    }

    #[test]
    fn test_check_response_500() {
        let err = EfiClient::check_response(reqwest::StatusCode::INTERNAL_SERVER_ERROR, "error")
            .unwrap_err();
        match err {
            ProviderError::Http { status, message } => {
                assert_eq!(status, 500);
                assert_eq!(message, "error");
            }
            _ => panic!("unexpected error type"),
        }
    }

    #[test]
    fn test_check_response_503() {
        let err = EfiClient::check_response(reqwest::StatusCode::SERVICE_UNAVAILABLE, "down")
            .unwrap_err();
        match err {
            ProviderError::Http { status, .. } => assert_eq!(status, 503),
            _ => panic!("unexpected error type"),
        }
    }

    // --- is_retryable ---

    #[test]
    fn test_retryable_rate_limited() {
        assert!(EfiClient::is_retryable(&ProviderError::RateLimited {
            retry_after_secs: 30
        }));
    }

    #[test]
    fn test_retryable_timeout() {
        assert!(EfiClient::is_retryable(&ProviderError::Timeout(10)));
    }

    #[test]
    fn test_retryable_503() {
        assert!(EfiClient::is_retryable(&ProviderError::Http {
            status: 503,
            message: "".into()
        }));
    }

    #[test]
    fn test_retryable_502() {
        assert!(EfiClient::is_retryable(&ProviderError::Http {
            status: 502,
            message: "".into()
        }));
    }

    #[test]
    fn test_retryable_network() {
        assert!(EfiClient::is_retryable(&ProviderError::Network(
            "err".into()
        )));
    }

    #[test]
    fn test_not_retryable_401() {
        assert!(!EfiClient::is_retryable(&ProviderError::Authentication(
            "".into()
        )));
    }

    #[test]
    fn test_not_retryable_404() {
        assert!(!EfiClient::is_retryable(&ProviderError::NotFound(
            "".into()
        )));
    }

    #[test]
    fn test_not_retryable_400() {
        assert!(!EfiClient::is_retryable(&ProviderError::Http {
            status: 400,
            message: "".into()
        }));
    }

    #[test]
    fn test_not_retryable_500() {
        assert!(!EfiClient::is_retryable(&ProviderError::Http {
            status: 500,
            message: "".into()
        }));
    }

    #[test]
    fn test_not_retryable_serialization() {
        assert!(!EfiClient::is_retryable(&ProviderError::Serialization(
            "".into()
        )));
    }

    // --- generate_txid tests ---

    #[test]
    fn test_generate_txid_starts_with_pix() {
        let txid = generate_txid();
        assert!(txid.starts_with("pix"));
    }

    #[test]
    fn test_generate_txid_length() {
        let txid = generate_txid();
        assert!(
            txid.len() >= 26 && txid.len() <= 35,
            "txid len: {}",
            txid.len()
        );
    }

    #[test]
    fn test_generate_txid_alphanumeric() {
        let txid = generate_txid();
        assert!(txid.chars().all(|c| c.is_ascii_alphanumeric()));
    }

    #[test]
    fn test_generate_txid_unique() {
        let t1 = generate_txid();
        let t2 = generate_txid();
        assert_ne!(t1, t2);
    }

    // --- to_pix_charge ---

    #[test]
    fn test_to_pix_charge_no_debtor() {
        let resp = make_charge_response("ATIVA");
        let charge = resp.to_pix_charge();
        assert!(charge.debtor.is_none());
        assert!(charge.e2eids.is_empty());
        assert!(charge.brcode.is_none());
    }

    #[test]
    fn test_to_pix_charge_with_debtor_cpf() {
        let resp = EfiChargeResponse {
            devedor: Some(EfiDevedor {
                nome: "João".to_string(),
                cpf: Some("12345678900".to_string()),
                cnpj: None,
            }),
            ..make_charge_response("ATIVA")
        };
        let charge = resp.to_pix_charge();
        assert!(charge.debtor.is_some());
        assert_eq!(charge.debtor.as_ref().unwrap().document, "12345678900");
    }

    #[test]
    fn test_to_pix_charge_with_debtor_cnpj() {
        let resp = EfiChargeResponse {
            devedor: Some(EfiDevedor {
                nome: "Empresa".to_string(),
                cpf: None,
                cnpj: Some("11222333000181".to_string()),
            }),
            ..make_charge_response("ATIVA")
        };
        let charge = resp.to_pix_charge();
        assert_eq!(charge.debtor.as_ref().unwrap().document, "11222333000181");
    }

    #[test]
    fn test_to_pix_charge_with_payments() {
        let resp = EfiChargeResponse {
            pix: Some(vec![
                EfiPixPayment {
                    end_to_end_id: "E1".to_string(),
                },
                EfiPixPayment {
                    end_to_end_id: "E2".to_string(),
                },
            ]),
            ..make_charge_response("CONCLUIDA")
        };
        let charge = resp.to_pix_charge();
        assert_eq!(charge.e2eids.len(), 2);
        assert_eq!(charge.e2eids[0], "E1");
        assert_eq!(charge.e2eids[1], "E2");
    }

    #[test]
    fn test_to_pix_charge_expires_at_calculated() {
        let now = Utc::now();
        let resp = EfiChargeResponse {
            calendario: EfiCalendarioResponse {
                criacao: now,
                expiracao: 7200,
            },
            ..make_charge_response("ATIVA")
        };
        let charge = resp.to_pix_charge();
        let expected = now + Duration::seconds(7200);
        assert!((charge.expires_at - expected).num_seconds().abs() < 2);
    }

    // --- convert_pix_transaction ---

    #[test]
    fn test_convert_pix_transaction_minimal() {
        let efi_tx = EfiPixTransaction {
            end_to_end_id: "E1".to_string(),
            txid: None,
            valor: "1.00".to_string(),
            horario: "2026-03-19T12:00:00Z".to_string(),
            info_pagador: None,
            pagador: None,
        };
        let tx = convert_pix_transaction(&efi_tx);
        assert_eq!(tx.e2eid, "E1");
        assert!(tx.txid.is_none());
        assert!(tx.payer_name.is_none());
        assert!(tx.payer_document.is_none());
    }

    #[test]
    fn test_convert_pix_transaction_with_cnpj_payer() {
        let efi_tx = EfiPixTransaction {
            end_to_end_id: "E1".to_string(),
            txid: None,
            valor: "1.00".to_string(),
            horario: "2026-03-19T12:00:00Z".to_string(),
            info_pagador: None,
            pagador: Some(EfiPagador {
                cpf: None,
                cnpj: Some("11222333000181".to_string()),
                nome: Some("Empresa".to_string()),
            }),
        };
        let tx = convert_pix_transaction(&efi_tx);
        assert_eq!(tx.payer_document, Some("11222333000181".to_string()));
    }

    #[test]
    fn test_convert_pix_transaction_bad_timestamp_uses_current() {
        let efi_tx = EfiPixTransaction {
            end_to_end_id: "E1".to_string(),
            txid: None,
            valor: "1.00".to_string(),
            horario: "not-a-date".to_string(),
            info_pagador: None,
            pagador: None,
        };
        let tx = convert_pix_transaction(&efi_tx);
        // Should not panic, uses Utc::now() fallback
        assert!((Utc::now() - tx.timestamp).num_seconds().abs() < 5);
    }

    // --- WebhookInfo ---

    #[test]
    fn test_webhook_info_deserialize() {
        let json = r#"{"webhookUrl":"https://example.com/webhook","chave":"key@test.com","criacao":"2026-03-19T00:00:00Z"}"#;
        let info: WebhookInfo = serde_json::from_str(json).unwrap();
        assert_eq!(info.webhook_url, "https://example.com/webhook");
        assert_eq!(info.chave, Some("key@test.com".to_string()));
        assert_eq!(info.created_at, Some("2026-03-19T00:00:00Z".to_string()));
    }

    #[test]
    fn test_webhook_info_serialize_roundtrip() {
        let info = WebhookInfo {
            webhook_url: "https://example.com".to_string(),
            chave: Some("key".to_string()),
            created_at: Some("2026-01-01".to_string()),
        };
        let json = serde_json::to_string(&info).unwrap();
        let back: WebhookInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(back.webhook_url, info.webhook_url);
    }

    #[test]
    fn test_webhook_info_minimal() {
        let json = r#"{"webhookUrl":"https://example.com"}"#;
        let info: WebhookInfo = serde_json::from_str(json).unwrap();
        assert_eq!(info.webhook_url, "https://example.com");
        assert!(info.chave.is_none());
        assert!(info.created_at.is_none());
    }
}
