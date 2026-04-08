export type CriterionStatus = 'met' | 'unmet' | 'indeterminate' | 'blocked'

export type TruthKind = 'job' | 'policy' | 'module-local'

export type TruthCatalogItem = {
	key: string
	display_name: string
	kind: TruthKind
	summary: string
	feature_path: string
	actor_roles: string[]
	approval_points: string[]
	desired_outcomes: string[]
	guardrails: string[]
	modules: Array<{ module_key: string; capability: string }>
	gherkin?: string
}

export type CriterionSummary = {
	criterion_id: string
	description: string
	required: boolean
	status: CriterionStatus
	detail?: string
	approval_ref?: string
	evidence_fact_ids: string[]
}

export type ExecutionSummary = {
	converged: boolean
	cycles: number
	stop_reason: string
	criteria: CriterionSummary[]
	experience_event_kinds: string[]
}

export type ProjectionSummary = {
	persisted: boolean
	organization_id?: string
	person_id?: string
	opportunity_id?: string
	subscription_id?: string
	workflow_case_ids: string[]
	document_ids: string[]
	fact_ids: string[]
	entitlement_ids: string[]
	approval_ids: string[]
	projected_event_kinds: string[]
}

export type TruthExecutionResponse = {
	truth: TruthCatalogItem
	execution: ExecutionSummary
	projection?: ProjectionSummary
}

export type TimelineEntry = {
	id: string
	kind: string
	summary: string
	actor: string
	timestamp: string
}

export type WorkflowCase = {
	id: string
	definition_key: string
	title: string
	state: string
	created_at: string
	priority: string
}

export type Organization = {
	id: string
	name: string
	lifecycle: string
	website?: string
	industry?: string
	owner_user_id?: string
	tags: string[]
}

export type JobState = {
	truth: TruthCatalogItem
	execution: ExecutionSummary
	projection?: ProjectionSummary
	executed_at: string
}
