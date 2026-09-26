use super::*;

#[derive(Clone, Default, Deserialize, PartialEq, utoipa::IntoParams)]
#[into_params(parameter_in = Query)]
#[serde(deny_unknown_fields)]
pub(crate) struct ApiPasteQuery {
    /// One-based result page. Defaults to 1.
    #[param(minimum = 1, default = 1)]
    page: Option<u32>,
    /// Results per page. Defaults to 30; the maximum is 100.
    #[param(minimum = 1, maximum = 100, default = 30)]
    page_size: Option<u32>,
    /// Case-insensitive search across paste ID, title, content, language, and attachment filename. Administrative listings also search owner usernames.
    q: Option<String>,
    /// Visibility identifier advertised by `/capabilities`.
    visibility: Option<String>,
    /// Restrict results to resources owned by the authenticated user.
    owner: Option<OwnerFilter>,
    /// Administrative owner-ID filter. Hidden from the public listing contract.
    #[param(ignore)]
    owner_id: Option<i64>,
    /// Positive folder ID. Requires `owner=me` and cannot be combined with `unfiled=true`.
    #[param(minimum = 1)]
    folder_id: Option<i64>,
    /// Restrict results to pastes without a folder. Requires `owner=me` when true and cannot be combined with `folder_id`.
    unfiled: Option<bool>,
    /// Content format identifier advertised by `/capabilities`.
    format: Option<String>,
    /// Syntax identifier advertised by `/languages`.
    language: Option<String>,
    /// Restrict results according to whether at least one attachment exists.
    has_attachments: Option<bool>,
    /// Inclusive lower bound on creation time, expressed as RFC 3339.
    #[param(format = DateTime)]
    created_after: Option<String>,
    /// Inclusive upper bound on creation time, expressed as RFC 3339.
    #[param(format = DateTime)]
    created_before: Option<String>,
    /// Restrict results by whether expiration is scheduled.
    expiration: Option<ExpirationFilter>,
    /// Inclusive minimum read count.
    #[param(minimum = 0)]
    min_reads: Option<i64>,
    /// Inclusive maximum read count.
    #[param(minimum = 0)]
    max_reads: Option<i64>,
    /// Inclusive minimum total size in bytes, including attachments.
    #[param(minimum = 0)]
    min_size_bytes: Option<i64>,
    /// Inclusive maximum total size in bytes, including attachments.
    #[param(minimum = 0)]
    max_size_bytes: Option<i64>,
    /// Restrict results according to whether a read limit is configured.
    read_limit: Option<ReadLimitFilter>,
    /// Field used to order results. Defaults to `created`.
    #[param(default = "created")]
    sort: Option<PasteSort>,
    /// Sort direction. Defaults to `desc`.
    #[param(default = "desc")]
    direction: Option<SortDirection>,
}

#[derive(Clone, Deserialize, PartialEq, utoipa::ToSchema)]
#[serde(rename_all = "lowercase")]
pub(crate) enum OwnerFilter {
    Me,
}

#[derive(Clone, Deserialize, PartialEq, utoipa::ToSchema)]
#[serde(rename_all = "lowercase")]
pub(crate) enum ExpirationFilter {
    Never,
    Scheduled,
}

#[derive(Clone, Deserialize, PartialEq, utoipa::ToSchema)]
#[serde(rename_all = "lowercase")]
pub(crate) enum ReadLimitFilter {
    Unlimited,
    Limited,
}

#[derive(Clone, Deserialize, PartialEq, utoipa::ToSchema)]
#[serde(rename_all = "lowercase")]
pub(crate) enum PasteSort {
    Created,
    Modified,
    Title,
    Reads,
    Expires,
    Size,
}

#[derive(Clone, Deserialize, PartialEq, utoipa::ToSchema)]
#[serde(rename_all = "lowercase")]
pub(crate) enum SortDirection {
    Asc,
    Desc,
}

macro_rules! filter_as_str {
    ($type:ty, {$($variant:path => $value:literal),+ $(,)?}) => {
        impl $type {
            fn as_str(&self) -> &'static str {
                match self { $($variant => $value),+ }
            }
        }
    };
}

filter_as_str!(ExpirationFilter, {
    ExpirationFilter::Never => "never",
    ExpirationFilter::Scheduled => "scheduled",
});
filter_as_str!(ReadLimitFilter, {
    ReadLimitFilter::Unlimited => "unlimited",
    ReadLimitFilter::Limited => "limited",
});
filter_as_str!(PasteSort, {
    PasteSort::Created => "created",
    PasteSort::Modified => "modified",
    PasteSort::Title => "title",
    PasteSort::Reads => "reads",
    PasteSort::Expires => "expires",
    PasteSort::Size => "size",
});
filter_as_str!(SortDirection, {
    SortDirection::Asc => "asc",
    SortDirection::Desc => "desc",
});

impl ApiPasteQuery {
    pub(super) fn into_internal(self) -> Result<PasteQuery, String> {
        if self.owner_id.is_some() {
            return Err("owner_id is available only on the administrative listing".into());
        }
        self.into_internal_with_owner(None)
    }

    pub(crate) fn into_admin_internal(self) -> Result<PasteQuery, String> {
        if self.owner.is_some() && self.owner_id.is_some() {
            return Err("owner and owner_id cannot be combined".into());
        }
        let owner_id = self.owner_id;
        self.into_internal_with_owner(owner_id)
    }

    fn into_internal_with_owner(self, owner_id: Option<i64>) -> Result<PasteQuery, String> {
        if self.page == Some(0) {
            return Err("page must be at least 1".into());
        }
        if self
            .page_size
            .is_some_and(|value| !(1..=crate::limits::MAX_PAGE_SIZE).contains(&value))
        {
            return Err(format!(
                "page_size must be between 1 and {}",
                crate::limits::MAX_PAGE_SIZE
            ));
        }
        let mine = self.owner.is_some();
        Ok(PasteQuery {
            page: self.page,
            page_size: self.page_size,
            search: self.q,
            visibility: self.visibility,
            owner_id,
            folder_id: self.folder_id,
            unfiled: self.unfiled,
            mine: Some(mine),
            content_kind: self.format,
            language: self.language,
            has_attachments: self.has_attachments,
            created_after: self
                .created_after
                .map(|value| contract::parse_timestamp(&value))
                .transpose()?,
            created_before: self
                .created_before
                .map(|value| contract::parse_timestamp(&value))
                .transpose()?,
            expiration: self.expiration.map(|value| value.as_str().into()),
            min_reads: self.min_reads,
            max_reads: self.max_reads,
            min_size_bytes: self.min_size_bytes,
            max_size_bytes: self.max_size_bytes,
            read_limit: self.read_limit.map(|value| value.as_str().into()),
            sort: self.sort.map(|value| value.as_str().into()),
            direction: self.direction.map(|value| value.as_str().into()),
        })
    }
}

#[cfg(test)]
mod paste_query_tests {
    use super::ApiPasteQuery;

    #[test]
    fn owner_id_is_accepted_only_for_administrative_listings() {
        let admin: ApiPasteQuery = serde_urlencoded::from_str("owner_id=2&page=3").unwrap();
        let internal = admin.into_admin_internal().unwrap();
        assert_eq!(internal.owner_id, Some(2));
        assert_eq!(internal.page, Some(3));

        let public: ApiPasteQuery = serde_urlencoded::from_str("owner_id=2").unwrap();
        assert!(public.into_internal().is_err());
    }

    #[test]
    fn administrative_owner_filters_cannot_be_ambiguous() {
        let query: ApiPasteQuery = serde_urlencoded::from_str("owner=me&owner_id=2").unwrap();
        assert!(query.into_admin_internal().is_err());
    }
}
