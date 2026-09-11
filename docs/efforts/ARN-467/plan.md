# Implementation plan

1. Find out why every git request 504s, by instrumenting the path rather than
   inferring from the symptom.
2. Permit, per module and by name, the four things a git guest must do: call its
   own kernel's loopback origin, read the `blob_endpoint` secret, read and write
   the git-object and field-overflow blob namespaces, and resolve a GitToken.
3. Derive each guest's OData base from that secret instead of the `Host` header.
4. Pin the kernel that carries the matching fixes (ARN-499, temper PR #463) and
   raise the bundle byte budget to a size a real app reaches.
5. Prove the whole surface live against Genesis production: clone, fetch,
   authenticated push, bundle serve, REST, and ref advance — each as the request
   an agent would actually make, with a credential, against the deployed host.
