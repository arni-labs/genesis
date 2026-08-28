# bundle-read
The version bundle endpoint serves a pinned hash's files.
Drive: GET /api/genesis/apps/{owner}/{name}/versions/{hash}/bundle ;
pass = 200 + non-empty files for a known hash.
