export type Section = 'jobs' | 'accounts' | 'workflows' | 'approvals' | 'system'

export const navSections: Array<{ id: Section; label: string }> = [
	{ id: 'jobs', label: 'Jobs' },
	{ id: 'accounts', label: 'Accounts' },
	{ id: 'workflows', label: 'Workflows' },
	{ id: 'approvals', label: 'Approvals' },
	{ id: 'system', label: 'System' }
]

export type TruthKind = 'job' | 'policy' | 'module-local'

export type OrganizationLifecycle = 'prospect' | 'active' | 'dormant' | 'partner'

export type OpportunityStage =
	| 'qualifying'
	| 'discovery'
	| 'proposal'
	| 'negotiation'
	| 'closed-won'
	| 'closed-lost'

export type SubscriptionStatus =
	| 'draft'
	| 'pending-activation'
	| 'active'
	| 'suspended'
	| 'cancelled'

export type CatalogPlanKind = 'subscription' | 'prepaid-credits' | 'enterprise-custom'

export type BillingPeriod = 'monthly' | 'quarterly' | 'annual' | 'one-time' | 'custom'

export type TimelineEventKind =
	| 'activity'
	| 'note'
	| 'document'
	| 'communication'
	| 'fact'
	| 'audit'

export type ApprovalStatus = 'pending' | 'approved' | 'rejected'

export type WorkflowState =
	| 'open'
	| 'awaiting-approval'
	| 'waiting-external'
	| 'blocked'
	| 'done'

export type WorkflowPriority = 'low' | 'medium' | 'high' | 'critical'

export type ExecutionState = 'idle' | 'running' | 'completed' | 'blocked' | 'failed'

export type CriterionStatus = 'met' | 'unmet' | 'indeterminate' | 'blocked'

export type RecordKind =
	| 'organization'
	| 'person'
	| 'relationship'
	| 'lead'
	| 'opportunity'
	| 'conversation'
	| 'activity'
	| 'task'
	| 'offer-quote'
	| 'order-subscription'
	| 'document'
	| 'fact'
	| 'intent'
	| 'workflow-case'
	| 'communication-event'
	| 'permission-grant'
	| 'audit-entry'
	| 'note'
	| 'catalog-item'

export type TruthListItem = {
	key: string
	display_name: string
	kind: TruthKind
	summary: string
	packs: string[]
	executable: boolean
}

export type OrganizationListItem = {
	id: string
	name: string
	lifecycle: OrganizationLifecycle
	website?: string
	people_count: number
	open_opportunity_count: number
	updated_at: string
}

export type OpportunityListItem = {
	id: string
	organization_name: string
	name: string
	stage: OpportunityStage
	value_minor: number
	currency_code: string
	confidence_bps: number
	next_step?: string
	updated_at: string
}

export type SubscriptionListItem = {
	id: string
	organization_id: string
	organization_name: string
	status: SubscriptionStatus
	catalog_item_id?: string
	catalog_item_name?: string
	value_minor: number
	currency_code: string
	started_at: string
	activated_at?: string
}

export type CatalogItemListItem = {
	id: string
	sku: string
	name: string
	description?: string
	plan_kind: CatalogPlanKind
	active: boolean
	billing_period?: BillingPeriod
	price_minor?: number
	currency_code?: string
	entitlements_summary: string[]
}

export type TimelineEventItem = {
	id: string
	kind: TimelineEventKind
	summary: string
	actor: string
	timestamp: string
}

export type ApprovalListItem = {
	id: string
	truth_key: string
	reason: string
	created_at: string
	status: ApprovalStatus
}

export type WorkflowCaseListItem = {
	id: string
	definition_key: string
	title: string
	state: WorkflowState
	created_at: string
	priority: WorkflowPriority
}

export type OperatorDashboard = {
	jobs: TruthListItem[]
	approvals: ApprovalListItem[]
	exceptions: WorkflowCaseListItem[]
	recent_timeline: TimelineEventItem[]
}

export type TruthExecutionResult = {
	converged: boolean
	cycles: number
	stop_reason: string
	experience_event_kinds: string[]
}

export type TruthExecutionProjection = {
	organization_id?: string
	person_id?: string
	opportunity_id?: string
	subscription_id?: string
	workflow_case_ids: string[]
	approval_ids: string[]
	fact_ids: string[]
	document_ids: string[]
	entitlement_ids: string[]
	projected_event_kinds: string[]
}

export type CriteriaOutcomeItem = {
	criterion_id: string
	description: string
	required: boolean
	status: CriterionStatus
	detail?: string
	approval_ref?: string
	evidence_fact_ids: string[]
}

export type TruthExecutionSession = {
	truth_key: string
	state: ExecutionState
	result?: TruthExecutionResult
	criteria_outcomes: CriteriaOutcomeItem[]
	projection?: TruthExecutionProjection
	error?: string
}

export type AccountWorkspaceSummary = {
	organization: {
		id: string
		name: string
		lifecycle: OrganizationLifecycle
		website?: string
		industry?: string
		owner_user_id?: string
		tags: string[]
	}
	people: Array<{
		id: string
		full_name: string
		title?: string
		email?: string
	}>
	opportunities: OpportunityListItem[]
	subscriptions: SubscriptionListItem[]
	entitlements: Array<{
		id: string
		key: string
		value_summary: string
	}>
	recent_timeline: TimelineEventItem[]
}

export type SystemProfile = {
	modules: Array<{ key: string; display_name: string; suite: string }>
	feature_toggles: {
		analytics_enabled: boolean
		optimization_enabled: boolean
		llm_enabled: boolean
		runtime_modules: Array<{ name: string; enabled: boolean; purpose: string }>
		supported_truth_keys: string[]
	}
}

export type TruthExecutionInputs = Record<string, string>

export type TruthInputFieldType = 'text' | 'textarea' | 'email' | 'url' | 'number' | 'boolean'

export type TruthInputFieldSchema = {
	key: string
	label: string
	type: TruthInputFieldType
	required: boolean
	description?: string
	placeholder?: string
	defaultValue?: string | number | boolean
}

export type TruthInputSchema = {
	truth_key: string
	title: string
	description: string
	fields: TruthInputFieldSchema[]
	presets?: Array<{
		label: string
		description: string
		values: TruthExecutionInputs
	}>
}

export const truthInputSchemas: Record<string, TruthInputSchema> = {
	'qualify-inbound-lead': {
		truth_key: 'qualify-inbound-lead',
		title: 'Qualify Inbound Lead',
		description: 'Capture an inbound commercial signal and shape the next governed operator step.',
		fields: [
			{
				key: 'organization_name',
				label: 'Organization Name',
				type: 'text',
				required: true,
				placeholder: 'Northwind'
			},
			{
				key: 'inbound_summary',
				label: 'Inbound Summary',
				type: 'textarea',
				required: true,
				placeholder: 'Summarize the buyer signal, JTBD, and commercial context.'
			},
			{
				key: 'contact_name',
				label: 'Primary Contact',
				type: 'text',
				required: false,
				placeholder: 'Alice Doe'
			},
			{
				key: 'contact_title',
				label: 'Contact Title',
				type: 'text',
				required: false,
				placeholder: 'CTO'
			},
			{
				key: 'contact_email',
				label: 'Contact Email',
				type: 'email',
				required: false,
				placeholder: 'alice@northwind.example'
			},
			{
				key: 'website',
				label: 'Website',
				type: 'url',
				required: false,
				placeholder: 'https://northwind.example'
			},
			{
				key: 'owner_user_id',
				label: 'Suggested Owner',
				type: 'text',
				required: false,
				placeholder: 'kenneth'
			},
			{
				key: 'next_step',
				label: 'Next Step',
				type: 'text',
				required: false,
				placeholder: 'Schedule qualification review'
			},
			{
				key: 'opportunity_value_minor',
				label: 'Opportunity Value Minor',
				type: 'number',
				required: false,
				placeholder: '24000000'
			},
			{
				key: 'require_manual_review',
				label: 'Require Manual Review',
				type: 'boolean',
				required: false,
				description: 'Open an approval path instead of projecting a direct happy-path opportunity.',
				defaultValue: false
			},
			{
				key: 'manual_review_reason',
				label: 'Manual Review Reason',
				type: 'textarea',
				required: false,
				placeholder: 'Commercial terms exceed the standard path.'
			}
		],
		presets: [
			{
				label: 'Northwind Happy Path',
				description: 'Projects a qualified opportunity directly.',
				values: {
					organization_name: 'Northwind',
					inbound_summary: 'Champion asked for a governed CRM substrate and audit trail.',
					contact_name: 'Alice Doe',
					contact_title: 'CTO',
					contact_email: 'alice@northwind.example',
					website: 'https://northwind.example',
					industry: 'Software',
					owner_user_id: 'kenneth',
					next_step: 'Send architecture brief and qualification follow-up.',
					opportunity_value_minor: '24000000',
					require_manual_review: 'false',
					manual_review_reason: ''
				}
			},
			{
				label: 'Apex Labs Blocked Path',
				description: 'Opens a manual review workflow and pending approval.',
				values: {
					organization_name: 'Apex Labs',
					inbound_summary: 'Procurement path is non-standard and needs explicit review.',
					contact_name: 'Morgan Lee',
					contact_title: 'VP Operations',
					website: 'https://apex.example',
					owner_user_id: 'revops-queue',
					next_step: 'Open approval path and validate non-standard commercials.',
					opportunity_value_minor: '45000000',
					require_manual_review: 'true',
					manual_review_reason: 'Commercial terms exceed the standard qualification path.'
				}
			}
		]
	},
	'activate-subscription': {
		truth_key: 'activate-subscription',
		title: 'Activate Subscription',
		description:
			'Activate a pending subscription against a catalog plan once commercial confirmation is explicit.',
		fields: [
			{
				key: 'organization_id',
				label: 'Organization ID',
				type: 'text',
				required: true,
				placeholder: 'Copy from /revenue or /accounts'
			},
			{
				key: 'subscription_id',
				label: 'Subscription ID',
				type: 'text',
				required: true,
				placeholder: 'Copy from /revenue'
			},
			{
				key: 'catalog_item_id',
				label: 'Catalog Item ID',
				type: 'text',
				required: true,
				placeholder: 'Catalog plan to activate'
			},
			{
				key: 'payment_confirmed',
				label: 'Payment Confirmed',
				type: 'boolean',
				required: true,
				description: 'Unchecked will route the activation into a manual approval path.',
				defaultValue: false
			}
		]
	},
	'refill-prepaid-ai-credits': {
		truth_key: 'refill-prepaid-ai-credits',
		title: 'Refill Prepaid AI Credits',
		description:
			'Apply a prepaid credit top-up to an active subscription with explicit payment state.',
		fields: [
			{
				key: 'organization_id',
				label: 'Organization ID',
				type: 'text',
				required: true,
				placeholder: 'Copy from /revenue or /accounts'
			},
			{
				key: 'subscription_id',
				label: 'Subscription ID',
				type: 'text',
				required: true,
				placeholder: 'Active prepaid subscription'
			},
			{
				key: 'amount_minor',
				label: 'Amount Minor',
				type: 'number',
				required: true,
				placeholder: '150000'
			},
			{
				key: 'currency_code',
				label: 'Currency Code',
				type: 'text',
				required: true,
				placeholder: 'USD',
				defaultValue: 'USD'
			},
			{
				key: 'payment_reference',
				label: 'Payment Reference',
				type: 'text',
				required: true,
				placeholder: 'pay_demo_001'
			},
			{
				key: 'payment_status',
				label: 'Payment Status',
				type: 'text',
				required: true,
				description: 'Use confirmed, pending, or failed.',
				placeholder: 'confirmed',
				defaultValue: 'confirmed'
			}
		]
	}
}

export type OperatorShellData = {
	dashboard: OperatorDashboard
	truths: TruthListItem[]
	organizations: OrganizationListItem[]
	opportunities: OpportunityListItem[]
	workflows: WorkflowCaseListItem[]
	approvals: ApprovalListItem[]
	profile: SystemProfile
}
