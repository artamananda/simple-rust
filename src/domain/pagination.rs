//! Value object paginasi — dipakai bersama oleh service dan repository.

use super::error::DomainError;

/// Halaman yang diminta. Nilainya dijamin masuk akal (page >= 1,
/// 1 <= per_page <= 100) karena hanya bisa dibuat lewat [`Pagination::new`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pagination {
    page: u32,
    per_page: u32,
}

impl Pagination {
    pub const DEFAULT_PER_PAGE: u32 = 20;
    pub const MAX_PER_PAGE: u32 = 100;

    pub fn new(page: u32, per_page: u32) -> Result<Self, DomainError> {
        if page == 0 {
            return Err(DomainError::validation("page", "minimal 1"));
        }
        if per_page == 0 {
            return Err(DomainError::validation("perPage", "minimal 1"));
        }
        if per_page > Self::MAX_PER_PAGE {
            return Err(DomainError::validation(
                "perPage",
                format!("maksimal {}", Self::MAX_PER_PAGE),
            ));
        }
        Ok(Self { page, per_page })
    }

    pub fn page(&self) -> u32 {
        self.page
    }

    pub fn per_page(&self) -> u32 {
        self.per_page
    }

    /// LIMIT untuk query SQL.
    pub fn limit(&self) -> i64 {
        i64::from(self.per_page)
    }

    /// OFFSET untuk query SQL.
    pub fn offset(&self) -> i64 {
        i64::from(self.page - 1) * i64::from(self.per_page)
    }
}

impl Default for Pagination {
    fn default() -> Self {
        Self {
            page: 1,
            per_page: Self::DEFAULT_PER_PAGE,
        }
    }
}

/// Sepotong hasil query beserta total keseluruhan barisnya.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Page<T> {
    pub items: Vec<T>,
    pub total: i64,
    pub pagination: Pagination,
}

impl<T> Page<T> {
    pub fn new(items: Vec<T>, total: i64, pagination: Pagination) -> Self {
        Self {
            items,
            total,
            pagination,
        }
    }

    pub fn total_pages(&self) -> i64 {
        let per_page = i64::from(self.pagination.per_page());
        (self.total + per_page - 1) / per_page
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offset_dihitung_dari_nomor_halaman() {
        let pagination = Pagination::new(3, 20).expect("valid");
        assert_eq!(pagination.limit(), 20);
        assert_eq!(pagination.offset(), 40);
    }

    #[test]
    fn halaman_nol_ditolak() {
        assert!(Pagination::new(0, 20).is_err());
    }

    #[test]
    fn per_page_di_atas_batas_ditolak() {
        assert!(Pagination::new(1, Pagination::MAX_PER_PAGE + 1).is_err());
    }

    #[test]
    fn total_pages_membulatkan_ke_atas() {
        let page = Page::new(vec![1, 2, 3], 21, Pagination::new(1, 20).expect("valid"));
        assert_eq!(page.total_pages(), 2);
    }
}
