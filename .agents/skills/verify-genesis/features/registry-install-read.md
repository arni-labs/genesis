# registry-install-read
The pinned-ref bundle URL an install would fetch serves the advertised content.
Drive (read-only): read the app's LatestVersionHash from /tdata/Apps over OData,
then GET /api/genesis/apps/{owner}/{name}/versions/{hash}/bundle for that hash ;
pass = 200 + non-empty files matching the advertised hash. (Verifying that a NEW
publish moves this pointer belongs to publish-new-version, which mutates state.)
