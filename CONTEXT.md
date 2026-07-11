# Warp

Warp is a terminal and agentic development environment that can operate without a user account while retaining a non-account identity for supported services.

## Identity

**Account Sign-in**:
A user-initiated flow that associates Warp with a persistent personal or organization account.
_Avoid_: Login, authentication

**Anonymous Session**:
A non-account identity that lets a user access supported Warp features without Account Sign-in.
_Avoid_: Logged-out user, guest account

**Anonymous-only Mode**:
A product mode in which every user operates through an Anonymous Session and Account Sign-in is unavailable.
_Avoid_: Logged-out mode, temporary mode

## AI Providers

**OpenAI-compatible Provider**:
A user-configured AI provider identified by a Base URL, Model, and API Key and accessed through the OpenAI API protocol.
_Avoid_: Warp-managed model, account model
