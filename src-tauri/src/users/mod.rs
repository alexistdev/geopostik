//! Menu Pengguna: kelola akun, peran, password & PIN (hak USER_MANAGE). Pengguna tidak pernah
//! dihapus permanen (soft delete) dan setiap perubahan dicatat di log audit.

pub mod commands;
mod model;
mod repo;
mod service;

#[cfg(test)]
mod tests;
