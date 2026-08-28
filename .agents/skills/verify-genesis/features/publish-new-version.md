# publish-new-version
PublishNewVersion moves Apps.LatestVersionHash to the pushed commit.
Drive: push commit -> POST Temper.Git.PublishNewVersion -> GET Apps('id');
pass = LatestVersionHash == pushed hash (read back, not assumed).
