# Route anonymous AI through Warp Agent

Anonymous-only Mode will send OpenAI-compatible Provider settings through the existing Warp Agent request path instead of calling provider endpoints directly from the client. This preserves streaming, tool use, context, and agent orchestration while keeping provider credentials in local secure storage and attaching them only to requests that need them; an Anonymous Session supplies the service identity.

## Considered Options

Direct client-to-provider requests were rejected because they would bypass the existing agent protocol and require separate implementations of core agent behavior.
