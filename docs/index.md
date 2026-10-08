---
# https://vitepress.dev/reference/default-theme-home-page
layout: home

hero:
  name: "Cellworks"
  text: "Documentation"
  tagline: "A native desktop map that makes an engineer's projects, code, AI agent sessions, tickets, infrastructure and data navigable in one place."
  actions:
    - theme: brand
      text: Architecture
      link: /arc42/
    - theme: alt
      text: Domain model
      link: /domain/

features:
  - title: Architecture (arc42)
    details: System design, quality goals, building blocks, runtime and crosscutting concerns.
    link: /arc42/
    linkText: Read the architecture
  - title: Decisions (ADR)
    details: The architectural decisions that shaped the system, and why.
    link: /adr/
    linkText: Browse decisions
  - title: Domain
    details: The ubiquitous language, bounded contexts, and the context map.
    link: /domain/
    linkText: Explore the domain
  - title: Services
    details: Per-component coding and test conventions.
    link: /services/
    linkText: See conventions
---
