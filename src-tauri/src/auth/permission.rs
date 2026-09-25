use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::error::AppError;

/// Peran pengguna. Lihat docs/FLOW.md bagian 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[ts(export)]
pub enum Role {
    Owner,
    Pharmacist,
    Technician,
    Cashier,
}

impl Role {
    pub fn as_str(self) -> &'static str {
        match self {
            Role::Owner => "OWNER",
            Role::Pharmacist => "PHARMACIST",
            Role::Technician => "TECHNICIAN",
            Role::Cashier => "CASHIER",
        }
    }
}

impl std::str::FromStr for Role {
    type Err = AppError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "OWNER" => Ok(Role::Owner),
            "PHARMACIST" => Ok(Role::Pharmacist),
            "TECHNICIAN" => Ok(Role::Technician),
            "CASHIER" => Ok(Role::Cashier),
            other => Err(AppError::Internal(format!("peran tidak dikenal: {other}"))),
        }
    }
}

/// Hak akses. Keputusan akhir selalu diperiksa di command Rust.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[ts(export)]
pub enum Permission {
    SaleCreate,
    ShiftManage,
    ReceiptReprint,
    SellHardDrug,
    PrescriptionInput,
    PrescriptionValidate,
    ProductManage,
    PriceManage,
    PurchaseReceive,
    SupplierDebtManage,
    StockCountInput,
    StockCountApprove,
    StockDestroy,
    TransactionVoid,
    DiscountOverride,
    ViewCost,
    ReportSales,
    ReportSalesOwnShift,
    ReportSipnap,
    UserManage,
    SettingsManage,
    BackupManage,
    AuditView,
}

/// Hak apoteker yang bisa diatur pemilik lewat pengaturan.
#[derive(Debug, Clone, Copy)]
pub struct AccessSettings {
    pub pharmacist_can_view_cost: bool,
    pub pharmacist_can_edit_price: bool,
}

impl Default for AccessSettings {
    fn default() -> Self {
        Self {
            pharmacist_can_view_cost: true,
            pharmacist_can_edit_price: true,
        }
    }
}

const CASHIER: &[Permission] = &[
    Permission::SaleCreate,
    Permission::ShiftManage,
    Permission::ReceiptReprint,
    Permission::ReportSalesOwnShift,
];

const TECHNICIAN_EXTRA: &[Permission] = &[
    Permission::PrescriptionInput,
    Permission::ProductManage,
    Permission::PurchaseReceive,
    Permission::StockCountInput,
];

const PHARMACIST_EXTRA: &[Permission] = &[
    Permission::SellHardDrug,
    Permission::PrescriptionValidate,
    Permission::StockCountApprove,
    Permission::StockDestroy,
    Permission::TransactionVoid,
    Permission::DiscountOverride,
    Permission::ReportSales,
    Permission::ReportSipnap,
];

// Pemilik sengaja tidak memegang kewenangan teknis kefarmasian
// (obat keras, validasi resep, pemusnahan).
const OWNER: &[Permission] = &[
    Permission::SaleCreate,
    Permission::ShiftManage,
    Permission::ReceiptReprint,
    Permission::ReportSalesOwnShift,
    Permission::PrescriptionInput,
    Permission::ProductManage,
    Permission::PriceManage,
    Permission::PurchaseReceive,
    Permission::SupplierDebtManage,
    Permission::StockCountInput,
    Permission::StockCountApprove,
    Permission::TransactionVoid,
    Permission::DiscountOverride,
    Permission::ViewCost,
    Permission::ReportSales,
    Permission::ReportSipnap,
    Permission::UserManage,
    Permission::SettingsManage,
    Permission::BackupManage,
    Permission::AuditView,
];

fn role_permissions(role: Role, access: AccessSettings) -> Vec<Permission> {
    match role {
        Role::Owner => OWNER.to_vec(),
        Role::Cashier => CASHIER.to_vec(),
        Role::Technician => [CASHIER, TECHNICIAN_EXTRA].concat(),
        Role::Pharmacist => {
            let mut p = [CASHIER, TECHNICIAN_EXTRA, PHARMACIST_EXTRA].concat();
            if access.pharmacist_can_view_cost {
                p.push(Permission::ViewCost);
            }
            if access.pharmacist_can_edit_price {
                p.push(Permission::PriceManage);
            }
            p
        }
    }
}

/// Gabungan hak dari semua peran user, terurut dan tanpa duplikat.
pub fn effective_permissions(roles: &[Role], access: AccessSettings) -> Vec<Permission> {
    roles
        .iter()
        .flat_map(|&r| role_permissions(r, access))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cost_is_hidden_from_cashier_and_technician() {
        let access = AccessSettings::default();
        for role in [Role::Cashier, Role::Technician] {
            let p = effective_permissions(&[role], access);
            assert!(!p.contains(&Permission::ViewCost), "{role:?}");
            assert!(!p.contains(&Permission::PriceManage), "{role:?}");
        }
    }

    #[test]
    fn pharmacist_cost_access_follows_settings() {
        let off = AccessSettings {
            pharmacist_can_view_cost: false,
            pharmacist_can_edit_price: false,
        };
        let p = effective_permissions(&[Role::Pharmacist], off);
        assert!(!p.contains(&Permission::ViewCost));
        assert!(!p.contains(&Permission::PriceManage));

        let p = effective_permissions(&[Role::Pharmacist], AccessSettings::default());
        assert!(p.contains(&Permission::ViewCost));
        assert!(p.contains(&Permission::PriceManage));
    }

    #[test]
    fn owner_has_no_pharmaceutical_authority_unless_also_pharmacist() {
        let owner = effective_permissions(&[Role::Owner], AccessSettings::default());
        assert!(!owner.contains(&Permission::SellHardDrug));
        assert!(!owner.contains(&Permission::StockDestroy));

        let both = effective_permissions(&[Role::Owner, Role::Pharmacist], AccessSettings::default());
        assert!(both.contains(&Permission::SellHardDrug));
        assert!(both.contains(&Permission::UserManage));
    }
}
