# Allow HTTP AI provider URLs

OpenAI-compatible Provider Base URLs may use either HTTP or HTTPS, including for remote hosts. This maximizes compatibility with self-hosted services, with the accepted consequence that HTTP can expose API Keys and request content in transit; the product must not imply that such connections are secure.
