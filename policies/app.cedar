// App registry policies: App and AppInstallation rows.
//
// The tenant operator (the deployment's own key) registers, publishes, forks
// and installs apps, and reads and installs bundles. The registry composites
// write their declared sub-writes under their action_context. The
// app_registry module's reads are in module_reads.cedar; the anonymous
// public-bundle path does not consult these policies. Commons mode adds the
// owner-scoped overlays in policies/commons/app.cedar.

permit(
  principal is Agent,
  action in [
    Action::"create", Action::"Create", Action::"read", Action::"list",
    Action::"Update", Action::"RegisterNewApp", Action::"PublishNewVersion",
    Action::"Fork", Action::"Install", Action::"Deprecate", Action::"Delete",
    Action::"read_app_bundle", Action::"install_app"
  ],
  resource is App
) when {
  principal has agent_type && principal.agent_type == "operator" &&
  principal has agentTypeVerified && principal.agentTypeVerified == true
};

permit(
  principal is Agent,
  action in [
    Action::"create", Action::"Create", Action::"read", Action::"list",
    Action::"MarkInstalled", Action::"MarkFailed"
  ],
  resource is AppInstallation
) when {
  principal has agent_type && principal.agent_type == "operator" &&
  principal has agentTypeVerified && principal.agentTypeVerified == true
};

permit(principal, action == Action::"Create", resource is App) when {
  principal has action_context &&
  (principal.action_context == "composite:App.RegisterNewApp" ||
   principal.action_context == "composite:App.Fork")
};

permit(principal, action == Action::"Update", resource is App) when {
  principal has action_context &&
  principal.action_context == "composite:App.PublishNewVersion"
};

permit(principal, action == Action::"Create", resource is AppInstallation) when {
  principal has action_context &&
  principal.action_context == "composite:App.Install"
};

// A TemperPaw kernel installs pinned apps from this registry with a
// credential of the genesis-registry-reader agent type. It reads App rows and
// bundles, and nothing else.
permit(
  principal == Agent::"genesis-registry-reader",
  action in [Action::"read_app_bundle", Action::"read"],
  resource is App
) when {
  principal has agent_type && principal.agent_type == "genesis-registry-reader" &&
  principal has agentTypeVerified && principal.agentTypeVerified == true
};

forbid(principal == Agent::"genesis-registry-reader", action, resource) unless {
  resource is App && action in [Action::"read_app_bundle", Action::"read"]
};
