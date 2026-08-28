# registry-install-read
After a publish, the pinned-ref bundle URL for the NEW hash serves the new content.
Drive: publish a version (see publish-new-version), then GET the bundle URL for
the new hash ; pass = 200 + the pushed file content, proving installs pin to it.
