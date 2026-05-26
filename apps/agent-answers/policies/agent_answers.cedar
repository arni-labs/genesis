permit(principal, action in [Action::"create", Action::"read", Action::"list", Action::"Configure", Action::"RecordAnswer", Action::"Accept"], resource is Question);
permit(principal, action in [Action::"create", Action::"read", Action::"list", Action::"Submit", Action::"Accept"], resource is Answer);
