# Platform Roadmap

## Domain Surface

When the public platform surface is ready, plan for `prio.ai` as the primary domain with service-oriented subdomains such as:

- `analytics.prio.ai`
- `crm.prio.ai`
- `hr.prio.ai`
- `plan.prio.ai`

This repository is the CRM-first starting point, not the end state.

## Likely Future Domains

### CRM And Revenue

- accounts, contacts, opportunities, notes, tasks, workflows, and usage signals

### Projects And Planning

- projects
- agile tasks
- kanban and gantt views
- timesheets
- project profitability

### Finance

- subscriptions
- invoicing
- deferred revenue and revenue recognition
- bookkeeping and reporting
- expense claims

### HR

- employee lifecycle
- leave and attendance
- payroll
- appraisals

### Support

- issue tracking
- SLA policy
- customer knowledge base

### Web And Portal

- company website and content
- customer portal
- invoice and ticket visibility
- project progress visibility

### Procurement

- suppliers and vendor management
- purchase orders
- material requests
- incoming invoices

## Architectural Consequence

The CRM kernel should stay modular enough that these later contexts can either:

- live as separate bounded contexts behind sibling services, or
- attach to the same metadata and workflow substrate without forcing a rewrite

That is why the current scaffold already separates:

- business state
- metadata and views
- workflow and trust
- usage ingestion
- runtime and optimization hooks

