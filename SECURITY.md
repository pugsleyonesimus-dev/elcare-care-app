# Security Policy

## Supported versions

| Version | Supported |
|---------|-----------|
| `main` (latest) | ✅ |
| Older branches | ❌ — please upgrade |

## Reporting a vulnerability

**Do not open a public GitHub issue for security vulnerabilities.**

Report privately via email:

**security@elcarehub.io**

Include as much detail as possible:

- A description of the vulnerability and its potential impact
- Steps to reproduce or proof-of-concept code
- Affected component(s): smart contract, indexer, frontend, CI/CD
- Your contact details for follow-up

### Response SLA

| Milestone | Target |
|-----------|--------|
| Acknowledgement | 48 hours |
| Triage and severity assessment | 5 business days |
| Fix or mitigation plan communicated | 14 business days |
| Public disclosure (coordinated) | After fix is deployed, or 90 days maximum |

We follow a **coordinated disclosure** model. We ask that you give us a
reasonable opportunity to remediate before publishing. We will credit reporters
in the release notes unless you prefer to remain anonymous.

## Scope

In scope:

- Smart contracts (`contracts/`)
- Indexer API (`indexer/`)
- Frontend application (`frontend/`)
- Deployment scripts (`scripts/`)
- CI/CD pipeline (`.github/`)

Out of scope:

- Third-party services (Stellar network, Pinata, Magic.link, Sentry)
- Testnet-only issues with no mainnet path
- Social engineering / phishing of end users

## Security hardening references

- [Threat Model](docs/THREAT_MODEL.md) — attack surface analysis
- [Incident Runbook](docs/INCIDENT_RUNBOOK.md) — response procedures
