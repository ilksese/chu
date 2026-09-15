# Product

<!-- impeccable:product-schema 1 -->

## Platform

web

## Users

Primary users are local developers managing AI coding-agent resources across multiple installed hosts.

## Product Purpose

Chu provides one local control plane for discovering, installing, importing, configuring, testing, and deploying Skills, MCP services, and custom Agents. Success means users can understand deployment state and make host-specific changes without manually editing each host configuration.

## Positioning

Chu keeps a central local resource store while preserving independent deployment state and native configuration formats for OpenCode, Claude Code, and Codex.

## Operating Context

Users work in a desktop application alongside local coding-agent tools. Primary workflows include browsing resources, installing or creating entries, enabling them per host, testing MCP configurations, resolving imported resources, and maintaining host paths or backups.

## Capabilities and Constraints

- Existing installation, import, deployment toggle, MCP test, path editing, refresh, and backup restoration behavior must remain intact.
- The home view is the primary browsing and navigation surface for Skills, MCP services, and Agents.
- Wails provides the desktop shell and Go backend; React, Vite, TypeScript, Tailwind CSS, and Magic UI provide the frontend.
- The interface is light-theme only.

## Brand Commitments

The product name is Chu. The binding visual authority is `DESIGN.md`: an energetic neo-brutalist SaaS language using warm white surfaces, electric-yellow accents, black outlines, restrained hard shadows, and red only for danger or error states.

## Evidence on Hand

The repository contains working frontend flows, a Wails backend, demo data, and the Chu logo asset. No customer claims, benchmarks, or commercial proof should be invented.

## Product Principles

- Keep local resource state legible at a glance.
- Preserve host independence while centralizing resource management.
- Make consequential configuration changes explicit and reversible.
- Favor direct manipulation and short paths over explanatory chrome.

## Accessibility & Inclusion

All primary workflows must remain keyboard operable, expose clear accessible names and states, retain visible focus, and respect reduced-motion preferences.
