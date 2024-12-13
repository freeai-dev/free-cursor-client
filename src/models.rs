use serde::{Deserialize, Deserializer, Serialize};
use std::fmt;
use time::OffsetDateTime;

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusResponse {
    pub subscriptions: Vec<UserSubscription>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Rfc3339DateTime(pub OffsetDateTime);

impl<'de> Deserialize<'de> for Rfc3339DateTime {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        use serde::de::Error;
        use time::{format_description::well_known::Rfc3339, UtcOffset};

        let s = String::deserialize(deserializer)?;
        let offset = OffsetDateTime::parse(&s, &Rfc3339)
            .map_err(|e| D::Error::custom(format!("Failed to parse datetime: {}", e)))?;

        let local_offset = UtcOffset::current_local_offset()
            .map_err(|e| D::Error::custom(format!("Failed to get local offset: {}", e)))?;

        Ok(Self(offset.to_offset(local_offset)))
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserSubscription {
    pub status: UserSubscriptionStatus,
    pub start_date: Rfc3339DateTime,
    pub end_date: Rfc3339DateTime,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum UserSubscriptionStatus {
    Active,
    Expired,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LoginResponse {
    Token(Token),
    Pending(bool),
    Expired(bool),
    Error(String),
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Token {
    pub id: i64,
    pub email: String,
    pub access_token: String,
    pub access_token_expired_at: String,
    pub refresh_token: String,
    pub refresh_token_expired_at: String,
    pub machine_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageResponse {
    pub packages: Vec<Package>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Package {
    pub id: i64,
    pub name: String,
    pub price: f64,
    pub duration: PackageDuration,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrderResponse {
    pub order: Order,
    pub token: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Order {
    pub id: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrderStatus {
    Pending,
    Completed,
    Cancelled,
}

impl fmt::Display for OrderStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OrderStatus::Pending => write!(f, "Pending"),
            OrderStatus::Completed => write!(f, "Completed"),
            OrderStatus::Cancelled => write!(f, "Cancelled"),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PackageDuration {
    Monthly,
    Quarterly,
    SemiAnnual,
    Annual,
}

impl fmt::Display for PackageDuration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PackageDuration::Monthly => write!(f, "Monthly"),
            PackageDuration::Quarterly => write!(f, "Quarterly"),
            PackageDuration::SemiAnnual => write!(f, "Semi-Annual"),
            PackageDuration::Annual => write!(f, "Annual"),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PaymentUrlResponse {
    pub url: String,
}
