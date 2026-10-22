// SPDX-License-Identifier: AGPL-3.0-or-later

use async_graphql::http::{MultipartOptions, receive_body};
use async_graphql_axum::rejection::GraphQLRejection;
use axum::body::to_bytes;
use axum::extract::Request;
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};

use crate::library::ARTWORK_MAX;

pub(super) async fn graphql_request(req: Request) -> Result<async_graphql::Request, Response> {
    let content_type = req.headers().get(header::CONTENT_TYPE).and_then(|v| v.to_str().ok()).map(str::to_owned);

    let body = to_bytes(req.into_body(), ARTWORK_MAX + 1024 * 1024)
        .await
        .map_err(|_| StatusCode::PAYLOAD_TOO_LARGE.into_response())?;

    let opts = MultipartOptions::default().max_file_size(ARTWORK_MAX);
    let req = receive_body(content_type, body.as_ref(), opts).await.map_err(|e| GraphQLRejection(e).into_response())?;

    if req.uploads.len() > 1 {
        return Err((StatusCode::BAD_REQUEST, "only one file can be uploaded at a time").into_response());
    }

    Ok(req)
}

#[cfg(test)]
mod tests {
    use axum::body::Body;
    use axum::http::Request as HttpRequest;

    use super::*;

    fn multipart(sizes: &[usize]) -> Request {
        let map = if sizes.len() == 1 {
            r#"{"0":["variables.image"]}"#
        } else {
            r#"{"0":["variables.image"],"1":["variables.other"]}"#
        };

        let mut body = format!(
            "--test\r\nContent-Disposition: form-data; name=\"operations\"\r\n\r\n{{\"query\":\"mutation($image:Upload,$other:Upload){{upload(image:$image)}}\",\"variables\":{{\"image\":null,\"other\":null}}}}\r\n--test\r\nContent-Disposition: form-data; name=\"map\"\r\n\r\n{map}\r\n"
        );

        for (i, size) in sizes.iter().enumerate() {
            body.push_str(&format!(
                "--test\r\nContent-Disposition: form-data; name=\"{i}\"; filename=\"image.png\"\r\nContent-Type: image/png\r\n\r\n{}\r\n",
                "x".repeat(*size)
            ));
        }

        body.push_str("--test--\r\n");

        HttpRequest::builder()
            .header(header::CONTENT_TYPE, "multipart/form-data; boundary=test")
            .body(Body::from(body))
            .unwrap()
    }

    #[tokio::test]
    async fn request_body_is_bounded_without_content_length() {
        let req = HttpRequest::builder().body(Body::from(vec![0; ARTWORK_MAX + 1024 * 1024 + 1])).unwrap();
        assert_eq!(graphql_request(req).await.err().unwrap().status(), StatusCode::PAYLOAD_TOO_LARGE);
    }

    #[tokio::test]
    async fn multipart_enforces_file_size_and_count() {
        assert_eq!(
            graphql_request(multipart(&[ARTWORK_MAX + 1])).await.err().unwrap().status(),
            StatusCode::PAYLOAD_TOO_LARGE
        );

        assert_eq!(graphql_request(multipart(&[1, 1])).await.err().unwrap().status(), StatusCode::BAD_REQUEST);
        let req = graphql_request(multipart(&[ARTWORK_MAX])).await.unwrap();
        assert_eq!(req.uploads.len(), 1);
        assert_eq!(req.uploads[0].content.len(), ARTWORK_MAX);
    }

    #[tokio::test]
    async fn json_reset_requests_still_parse() {
        let req = HttpRequest::builder()
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(r#"{"query":"mutation($image:Upload){upload(image:$image)}","variables":{"image":null}}"#))
            .unwrap();

        assert!(graphql_request(req).await.unwrap().uploads.is_empty());
    }
}
