export type Section =
	| 'jobs'
	| 'operator-control'
	| 'accounts'
	| 'workflows'
	| 'approvals'
	| 'pipeline'
	| 'system'

export const navSections: Array<{ id: Section; label: string; href?: string }> = [
	{ id: 'pipeline', label: 'Pipeline', href: '/pipeline' },
	{ id: 'jobs', label: 'Jobs' },
	{ id: 'operator-control', label: 'Operator Control' },
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

export type OrganismResolutionLevel = 'declarative' | 'structural' | 'semantic' | 'learned'

export type TruthReadinessConfirmation = {
	resource: string
	kind: string
	detail: string
}

export type TruthReadinessGap = {
	resource: string
	kind: string
	severity: string
	reason: string
	suggestion?: string
}

export type TruthReadinessView = {
	ready: boolean
	confirmed: TruthReadinessConfirmation[]
	gaps: TruthReadinessGap[]
}

export type OrganismPackRequirementView = {
	pack_name: string
	reason: string
	confidence_bps: number
	source: OrganismResolutionLevel | string
}

export type OrganismCapabilityRequirementView = {
	capability: string
	reason: string
	confidence_bps: number
	source: OrganismResolutionLevel | string
}

export type OrganismTruthResolutionView = {
	truth_key: string
	blueprint?: string
	packs: OrganismPackRequirementView[]
	capabilities: OrganismCapabilityRequirementView[]
	invariants: string[]
	levels_attempted: Array<OrganismResolutionLevel | string>
	levels_contributed: Array<OrganismResolutionLevel | string>
	completeness_confidence_bps: number
	readiness: TruthReadinessView
}

export type ConvergeTruthResolutionView = {
	truth_key: string
	runtime: string
	pack_ids: string[]
	approval_points: string[]
	intent_kind: string
	request: string
	required_success_criteria: string[]
	hard_constraints: string[]
}

export type TruthModuleTouchItem = {
	module_key: string
	responsibility: string
}

export type TruthDetailItem = {
	key: string
	display_name: string
	kind: TruthKind
	summary: string
	feature_path: string
	actor_roles: string[]
	approval_points: string[]
	desired_outcomes: string[]
	guardrails: string[]
	modules: TruthModuleTouchItem[]
	gherkin: string
	packs: string[]
	executable: boolean
	organism_resolution?: OrganismTruthResolutionView
	converge_resolution?: ConvergeTruthResolutionView
}

export type WorkbenchAppStatus = 'ready' | 'preview' | 'hidden'

export type WorkbenchAppKind = 'workspace' | 'utility' | 'review'

export type WorkbenchAppManifest = {
	id: string
	display_name: string
	route: string
	summary: string
	kind: WorkbenchAppKind
	status: WorkbenchAppStatus
	capability_keys: string[]
	truth_keys: string[]
}

export type ExpenseReportStatus =
	| 'draft'
	| 'submitted'
	| 'in-review'
	| 'export-pending'
	| 'approved'

export type ExpenseOcrStatus = 'pending' | 'scanned' | 'needs-review' | 'failed'

export type ExpenseReport = {
	id: string
	title: string
	employee_name: string
	employee_email: string
	status: ExpenseReportStatus
	currency_code: string
	total_minor: number
	description?: string
	submitted_at?: string
	booking_export_reference?: string
	created_at: string
	updated_at: string
}

export type ExpenseItem = {
	id: string
	report_id: string
	merchant: string
	amount: {
		currency_code: string
		amount_minor: number
	}
	category: string
	occurred_at: string
	description?: string
	capture_source: string
	receipt_document_id?: string
	ocr_status: ExpenseOcrStatus
	ocr_engine?: string
	extracted_summary?: string
	ocr_fields: Record<string, string>
	policy_flags: string[]
	created_at: string
	updated_at: string
}

export type ExpenseReceiptSample = {
	sample_id: string
	report_id?: string
	document_file: string
	original_file_name: string
	document_path: string
	reference_path: string
	document_type: string
	capture_type: string
	expense_candidate: boolean
	reference_status: string
	expected_fields: Record<string, string>
	notes: string[]
}

export type ExpenseOcrRunStatus = 'completed' | 'failed'

export type ExpenseOcrFieldComparison = {
	field: string
	expected: string
	actual: string
}

export type ExpenseOcrBenchmark = {
	matched_fields: number
	compared_fields: number
	missing_fields: ExpenseOcrFieldComparison[]
	mismatched_fields: ExpenseOcrFieldComparison[]
}

export type ExpenseOcrRun = {
	sample_id: string
	engine: string
	status: ExpenseOcrRunStatus
	implementation?: string
	fields: Record<string, string>
	raw_text?: string
	warnings: string[]
	metadata: Record<string, string>
	benchmark?: ExpenseOcrBenchmark
	error?: string
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

export type AdapterReceiptStatus = 'succeeded' | 'rejected'

export type JobVerdict = 'satisfied' | 'blocked' | 'exhausted' | 'invalid'

export type EvidenceReadinessStatus =
	| 'present'
	| 'missing'
	| 'disputed'
	| 'blocked'
	| 'concern'

export type OperatorLedgerRecordKind =
	| 'observation_adapter_receipt'
	| 'job_readiness_packet'
	| 'operator_decision_receipt'
	| 'approval_receipt'
	| 'plan_receipt'
	| 'execution_receipt'
	| 'action_receipt'
	| 'outcome_receipt'
	| 'corpus_snapshot_receipt'
	| 'evidence_window_receipt'
	| 'disagreement_receipt'
	| 'analyst_review_receipt'
	| 'narrative_claim_receipt'
	| 'canonical_story_receipt'
	| 'claim_review_receipt'
	| 'editorial_approval_receipt'
	| 'publication_boundary_receipt'
	| 'app_local_receipt'

export type ReceiptFamily =
	| 'common'
	| 'long_running_job'
	| 'temporal_evidence'
	| 'content_publication'
	| 'app_local'

export type AuthorityEffect = 'none'

export type JobEvidenceStatus = {
	clause_id: string
	clause_key: string
	label: string
	status: EvidenceReadinessStatus
	fact_ids: string[]
	evidence_refs: string[]
	trace_links: string[]
	concern_record_ids: string[]
}

export type FuzzyMembership = {
	label: string
	score_basis_points: number
}

export type FuzzyRuleActivation = {
	rule_id: string
	strength_basis_points: number
	conclusion: string
}

export type FuzzyDefuzzifiedScore = {
	method: string
	score_basis_points: number
	domain_min_basis_points: number
	domain_max_basis_points: number
	domain_steps: number
}

export type FuzzyReadinessTrace = {
	variable_key: string
	observed_value_basis_points: number
	memberships: FuzzyMembership[]
	activated_rules: FuzzyRuleActivation[]
	defuzzified_score?: FuzzyDefuzzifiedScore | null
}

export type JobReadinessPacket = {
	packet_id: string
	package_id: string
	truth_version: string
	domain_hint: string
	job_key: string
	subject_ref: string
	adapter_receipt_id: string
	adapter_status: AdapterReceiptStatus
	verdict: JobVerdict | null
	authorizes_domain_action: boolean
	evidence_status: JobEvidenceStatus[]
	fuzzy_trace?: FuzzyReadinessTrace | null
	verifier_forbidden_actions: string[]
	operator_actions: string[]
}

export type OperatorLedgerEntry = {
	entry_id: string
	sequence: number
	record_kind: OperatorLedgerRecordKind
	receipt_family: ReceiptFamily
	source_ref: string
	package_id: string
	truth_version: string
	domain_hint: string
	payload_hash: string
	backlink_ids: string[]
	authority_effect: AuthorityEffect
	summary: string
}

export type OperatorReceiptFamilyView = {
	family: ReceiptFamily
	purpose: string
	record_kinds: OperatorLedgerRecordKind[]
}

export type OperatorControlPreviewBacking = 'live-app-feed'

export type OperatorControlPreview = {
	packet: JobReadinessPacket
	ledger_entries: OperatorLedgerEntry[]
	receipt_families: OperatorReceiptFamilyView[]
	backing: OperatorControlPreviewBacking
	backing_label: string
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
	'submit-expense-report': {
		truth_key: 'submit-expense-report',
		title: 'Submit Expense Report',
		description:
			'Stage a receipt-backed expense report, route policy exceptions into approval, and mark export readiness.',
		fields: [
			{
				key: 'organization_name',
				label: 'Organization Name',
				type: 'text',
				required: true,
				placeholder: 'Outcome Workbench'
			},
			{
				key: 'report_title',
				label: 'Report Title',
				type: 'text',
				required: false,
				placeholder: 'April travel reimbursement'
			},
			{
				key: 'merchant_name',
				label: 'Merchant',
				type: 'text',
				required: false,
				placeholder: 'SJ Rail'
			},
			{
				key: 'category',
				label: 'Category',
				type: 'text',
				required: false,
				placeholder: 'travel'
			},
			{
				key: 'amount_minor',
				label: 'Amount Minor',
				type: 'number',
				required: true,
				placeholder: '12850'
			},
			{
				key: 'currency_code',
				label: 'Currency Code',
				type: 'text',
				required: false,
				placeholder: 'EUR',
				defaultValue: 'EUR'
			},
			{
				key: 'expense_date',
				label: 'Expense Date',
				type: 'text',
				required: false,
				placeholder: '2026-04-12'
			},
			{
				key: 'receipt_uri',
				label: 'Receipt URI',
				type: 'text',
				required: true,
				placeholder: 'file:///receipts/april-train.pdf'
			},
			{
				key: 'receipt_title',
				label: 'Receipt Title',
				type: 'text',
				required: false,
				placeholder: 'Receipt: SJ Rail'
			},
			{
				key: 'receipt_media_type',
				label: 'Receipt Media Type',
				type: 'text',
				required: false,
				placeholder: 'application/pdf',
				defaultValue: 'application/pdf'
			},
			{
				key: 'ocr_confidence_bps',
				label: 'OCR Confidence (bps)',
				type: 'number',
				required: false,
				placeholder: '9200'
			},
			{
				key: 'out_of_policy',
				label: 'Out Of Policy',
				type: 'boolean',
				required: false,
				description: 'Checked will route the report into manual approval.',
				defaultValue: false
			},
			{
				key: 'require_manual_review',
				label: 'Require Manual Review',
				type: 'boolean',
				required: false,
				description: 'Force a review gate even when OCR and policy are otherwise acceptable.',
				defaultValue: false
			},
			{
				key: 'manual_review_reason',
				label: 'Manual Review Reason',
				type: 'textarea',
				required: false,
				placeholder: 'Receipt confidence is low or policy rationale is missing.'
			}
		],
		presets: [
			{
				label: 'Travel Happy Path',
				description: 'Stages an export-ready receipt with no extra approval path.',
				values: {
					organization_name: 'Outcome Workbench',
					report_title: 'April travel reimbursement',
					merchant_name: 'SJ Rail',
					category: 'travel',
					amount_minor: '12850',
					currency_code: 'SEK',
					expense_date: '2026-04-12',
					receipt_uri: 'file:///receipts/sj-rail-april-12.pdf',
					receipt_title: 'Receipt: SJ Rail',
					receipt_media_type: 'application/pdf',
					ocr_confidence_bps: '9200',
					out_of_policy: 'false',
					require_manual_review: 'false',
					manual_review_reason: ''
				}
			},
			{
				label: 'Entertainment Review',
				description: 'Routes a low-confidence, out-of-policy expense into approval.',
				values: {
					organization_name: 'Outcome Workbench',
					report_title: 'Client dinner reimbursement',
					merchant_name: 'Maison du Port',
					category: 'entertainment',
					amount_minor: '98000',
					currency_code: 'EUR',
					expense_date: '2026-04-11',
					receipt_uri: 'file:///receipts/maison-du-port.jpeg',
					receipt_title: 'Receipt: Maison du Port',
					receipt_media_type: 'image/jpeg',
					ocr_confidence_bps: '6200',
					out_of_policy: 'true',
					require_manual_review: 'false',
					manual_review_reason: 'Entertainment spend exceeded the standard allowance.'
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
	apps: WorkbenchAppManifest[]
	dashboard: OperatorDashboard
	operatorControlPreviews: OperatorControlPreview[]
	truths: TruthListItem[]
	organizations: OrganizationListItem[]
	opportunities: OpportunityListItem[]
	workflows: WorkflowCaseListItem[]
	approvals: ApprovalListItem[]
	profile: SystemProfile
}
