use actix_web::{error, http::StatusCode};

pub mod attestationdoc;

impl error::ResponseError for UserError {
    fn error_response(&self) -> actix_web::HttpResponse<actix_web::body::BoxBody> {
        actix_web::HttpResponse::build(self.status_code())
            .insert_header(actix_web::http::header::ContentType::plaintext())
            .body(self.to_string())
    }

    fn status_code(&self) -> actix_web::http::StatusCode {
        match self {
            UserError::InternalServerError => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}
