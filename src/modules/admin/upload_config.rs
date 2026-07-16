pub use crate::modules::admin::presentation::{
    SaveUploadConfigRequest, UploadConfigResponse, UploadFileInput, UploadImageResponse,
};

#[cfg(test)]
#[path = "../../../tests/unit_src/src_modules_admin_upload_config_tests.rs"]
mod tests;
