import type {
  ConsoleAdapter,
  ConsoleCommandDescriptor,
  ConsoleFieldDescriptor,
  ConsoleSafetyRule,
} from '../types'

const localBearer = 'dev'

const commonSafety: ConsoleSafetyRule[] = [
  {
    id: 'no-ui-only-state',
    label: 'No UI-only process state',
    description: 'The console may store form drafts and selected ids, but process state must come from app APIs.',
  },
  {
    id: 'derived-aids-not-authority',
    label: 'Derived aids are not authority',
    description: 'Analytics, memory, solver, and prediction outputs may suggest operator action but cannot silently execute it.',
  },
  {
    id: 'real-live-no-fixtures',
    label: 'REAL LIVE has no fixture fallback',
    description: 'Missing credentials or upstream services must fail honestly instead of substituting mock data.',
  },
  {
    id: 'artifact-preserves-gaps',
    label: 'Artifacts preserve gaps',
    description: 'Final artifacts must retain dissent, unresolved work, failed checks, and policy refusals.',
  },
]

const inquiryIdField: ConsoleFieldDescriptor = {
  id: 'id',
  label: 'Inquiry id',
  kind: 'text',
  required: true,
}

const atlasRoomIdField: ConsoleFieldDescriptor = {
  id: 'room_id',
  label: 'Room id',
  kind: 'text',
  required: false,
}

export const quorumConsoleAdapter: ConsoleAdapter = {
  appId: 'quorum',
  displayName: 'Quorum Sense',
  routePrefix: '/quorum',
  subjectRefKind: 'inquiry',
  nouns: {
    run: 'inquiry',
    spec: 'inquiry contract',
    event: 'chain event',
    artifact: 'process receipt',
  },
  connection: {
    defaultBaseUrl: '/quorum',
    localBearer,
  },
  run: {
    open: {
      id: 'quorum.open-contracted-inquiry',
      label: 'Open contracted inquiry',
      authority: 'chain-recorded',
      request: {
        method: 'POST',
        path: '/inquiry/contracted',
        body: {
          core_question: '{core_question}',
          starting_hypothesis: '{starting_hypothesis}',
          organizer_thread: '{organizer_thread}',
          allowed_detours: ['evidence-seeking detours'],
          forced_return_points: ['return before decision'],
          forbidden_actions: ['suppress dissent', 'treat predictions as authority'],
          anonymity_policy: '{anonymity_policy}',
          actor_policy: ['human_participant', 'facilitator', 'ai_research_agent', 'ai_synthesis_agent'],
          research_policy: 'mid-run research allowed when it cites sources',
          scoring_policy: 'optional scoring rounds are allowed; scores are not decisions',
          decision_rule: '{decision_rule}',
          evidence_requirements: ['named owner for each material domain object'],
          dissent_threshold: 0.6,
          time_budget: '{time_budget}',
          prior_consultation_policy: 'optional',
          probe_budget: {
            participant_slots: 3,
            time_seconds: 600,
            uncertainty_tokens: 300,
          },
          initial_phase: 'problem_diverge',
          content_hash: null,
        },
      },
      form: {
        fields: [
          { id: 'core_question', label: 'Core question', kind: 'textarea', required: true },
          { id: 'starting_hypothesis', label: 'Starting hypothesis', kind: 'textarea' },
          { id: 'organizer_thread', label: 'Organizer thread', kind: 'textarea' },
          {
            id: 'anonymity_policy',
            label: 'Visibility',
            kind: 'select',
            defaultValue: 'pseudonymous',
            options: [
              { label: 'Named', value: 'named' },
              { label: 'Pseudonymous', value: 'pseudonymous' },
              { label: 'Anonymous to group', value: 'anonymous_to_group' },
              { label: 'Sealed until round close', value: 'sealed_until_round_close' },
            ],
          },
          {
            id: 'decision_rule',
            label: 'Decision rule',
            kind: 'select',
            defaultValue: 'no_decision',
            options: [
              { label: 'No decision', value: 'no_decision' },
              { label: 'Organizer decides', value: 'organizer_decides' },
              { label: 'Evidence threshold', value: 'evidence_threshold' },
              { label: 'Consent, not consensus', value: 'consent_not_consensus' },
            ],
          },
          { id: 'time_budget', label: 'Time budget', kind: 'text', defaultValue: '45m live run' },
        ],
      },
      expectedEventTypes: ['quorum.inquiry.opened'],
    },
    load: { id: 'quorum.load-inquiry', label: 'Load inquiry', path: '/inquiry/{id}' },
    events: { id: 'quorum.events', label: 'Inquiry events', path: '/inquiry/{id}/events' },
    live: { id: 'quorum.live', label: 'Live events', path: '/inquiry/{id}/live', transport: 'sse' },
    integrity: { id: 'quorum.integrity', label: 'Integrity proof', path: '/inquiry/{id}/integrity' },
    receipt: { id: 'quorum.receipt', label: 'Process receipt', path: '/inquiry/{id}/process-receipt' },
    outcome: { id: 'quorum.outcome', label: 'Outcome', path: '/inquiry/{id}/outcome' },
  },
  controls: [
    {
      id: 'quorum.rounds',
      label: 'Rounds and thread',
      commands: [
        command('quorum.start-next-round', 'Start next round', 'POST', '/inquiry/{id}/rounds/next', 'chain-recorded', [
          inquiryIdField,
        ]),
        command(
          'quorum.redirect',
          'Facilitator redirect',
          'POST',
          '/inquiry/{id}/redirects',
          'receipt-bearing',
          [
            inquiryIdField,
            { id: 'reason', label: 'Reason', kind: 'textarea', required: true },
            { id: 'from_state', label: 'From state', kind: 'text', required: true },
            { id: 'to_state', label: 'To state', kind: 'text', required: true },
            { id: 'organizer_rule_citation', label: 'Rule citation', kind: 'text' },
          ],
        ),
      ],
    },
    {
      id: 'quorum.subengagements',
      label: 'Research and probes',
      commands: [
        command('quorum.open-research', 'Open research task', 'POST', '/inquiry/{id}/research', 'chain-recorded', [
          inquiryIdField,
          { id: 'question', label: 'Question', kind: 'textarea', required: true },
          { id: 'scope', label: 'Scope', kind: 'text', required: true },
          { id: 'evidence_requirements', label: 'Evidence requirements', kind: 'string-list' },
        ]),
        command(
          'quorum.allocate-probes',
          'Allocate probes',
          'POST',
          '/inquiry/{id}/probes/allocate',
          'chain-recorded',
          [
            inquiryIdField,
            { id: 'participant_slots', label: 'Participant slots', kind: 'number', defaultValue: 3 },
            { id: 'time_seconds', label: 'Time seconds', kind: 'number', defaultValue: 600 },
            { id: 'uncertainty_tokens', label: 'Uncertainty tokens', kind: 'number', defaultValue: 300 },
          ],
          {
            budget: {
              participant_slots: '{participant_slots}',
              time_seconds: '{time_seconds}',
              uncertainty_tokens: '{uncertainty_tokens}',
            },
          },
        ),
      ],
    },
  ],
  aids: [
    {
      id: 'quorum.insights',
      label: 'SenseMap insights',
      aidKind: 'analytic',
      read: { id: 'quorum.insights.read', label: 'Read insights', path: '/sensemap/insights' },
      recompute: command('quorum.sensemap.recompute', 'Recompute SenseMap', 'POST', '/sensemap/recompute', 'derived-recompute'),
      authorityBoundary: 'Insights are derived aids and are not decision-citable evidence.',
    },
    {
      id: 'quorum.predictions',
      label: 'Anticipatory signals',
      aidKind: 'prediction',
      read: {
        id: 'quorum.predictions.read',
        label: 'Read anticipatory signals',
        path: '/sensemap/anticipatory-signals',
      },
      recompute: command(
        'quorum.predictions.detect',
        'Detect anticipatory signals',
        'POST',
        '/sensemap/anticipatory-signals/detect',
        'derived-recompute',
      ),
      authorityBoundary: 'Predictions require falsifiability and cannot authorize decisions.',
    },
  ],
  artifacts: [
    {
      id: 'quorum.process-receipt',
      label: 'Process receipt',
      resolverScheme: 'quorum://',
      read: { id: 'quorum.receipt.artifact', label: 'Process receipt', path: '/inquiry/{id}/process-receipt' },
      requiredProvenance: ['event_history_root', 'generated_at', 'decision_rule'],
    },
  ],
  safety: commonSafety,
}

export const atlasConsoleAdapter: ConsoleAdapter = {
  appId: 'atlas',
  displayName: 'Atlas Integration',
  routePrefix: '/atlas',
  subjectRefKind: 'acquisition-room',
  nouns: {
    run: 'acquisition room',
    spec: 'consolidation intent',
    event: 'room memory event',
    artifact: 'readiness packet',
  },
  connection: {
    defaultBaseUrl: '/atlas',
    localBearer,
  },
  run: {
    load: {
      id: 'atlas.readiness-room',
      label: 'Readiness room',
      path: '/v1/acquisition/readiness-room',
    },
    events: {
      id: 'atlas.room-memory',
      label: 'Room memory',
      path: '/v1/acquisition/room-memory',
    },
    receipt: {
      id: 'atlas.mosaic-readiness',
      label: 'Mosaic readiness',
      path: '/v1/mosaic/readiness',
    },
  },
  controls: [
    {
      id: 'atlas.room-memory',
      label: 'Room memory',
      commands: [
        command(
          'atlas.heartbeat',
          'Heartbeat room memory',
          'POST',
          '/v1/acquisition/room-memory/heartbeat',
          'chain-recorded',
          [atlasRoomIdField],
        ),
      ],
    },
    {
      id: 'atlas.quorum',
      label: 'Quorum uncertainty',
      commands: [
        command(
          'atlas.signal-quorum-question',
          'Signal Quorum question',
          'POST',
          '/v1/acquisition/quorum-uncertainty/{question_id}/signal',
          'chain-recorded',
          [
            { id: 'question_id', label: 'Question id', kind: 'text', required: true },
            {
              id: 'kind',
              label: 'Signal kind',
              kind: 'select',
              defaultValue: 'support',
              options: [
                { label: 'Support', value: 'support' },
                { label: 'Dispute', value: 'dispute' },
                { label: 'Evidence gap', value: 'evidence_gap' },
              ],
            },
          ],
        ),
      ],
    },
  ],
  aids: [
    {
      id: 'atlas.budget',
      label: 'Budget check',
      aidKind: 'solver',
      recompute: command(
        'atlas.budget.check',
        'Check Mosaic budget',
        'POST',
        '/v1/mosaic/budget/check',
        'derived-recompute',
      ),
      authorityBoundary: 'Budget checks constrain operator action but do not mutate integration state.',
    },
    {
      id: 'atlas.quorum-uncertainty',
      label: 'Quorum uncertainty',
      aidKind: 'memory',
      read: {
        id: 'atlas.quorum-uncertainty.read',
        label: 'Quorum unresolved questions',
        path: '/v1/acquisition/quorum-uncertainty',
      },
      authorityBoundary: 'Quorum questions are uncertainty inputs, not Atlas consolidation decisions.',
    },
  ],
  artifacts: [
    {
      id: 'atlas.readiness-packet',
      label: 'Readiness packet',
      resolverScheme: 'atlas://',
      read: { id: 'atlas.packet.read', label: 'Readiness room', path: '/v1/acquisition/readiness-room' },
      requiredProvenance: ['retrieved_at', 'source_refs'],
    },
  ],
  safety: commonSafety,
}

export const wardenConsoleAdapter: ConsoleAdapter = {
  appId: 'warden',
  displayName: 'Warden Compliance',
  routePrefix: '/warden',
  subjectRefKind: 'compliance-gate',
  nouns: {
    run: 'compliance review',
    spec: 'rule registry',
    event: 'verdict event',
    artifact: 'audit pack',
  },
  connection: {
    defaultBaseUrl: '/warden',
    localBearer,
  },
  run: {
    load: {
      id: 'warden.truths',
      label: 'Compliance truths',
      path: '/truths',
    },
    receipt: {
      id: 'warden.identity-gate',
      label: 'Identity data residency gate',
      path: '/v1/demo/acquisition-assets/shared-identity-core/gates/dd-evidence.identity-data-residency',
    },
  },
  controls: [
    {
      id: 'warden.gates',
      label: 'Gates and shadow checks',
      commands: [],
    },
  ],
  aids: [
    {
      id: 'warden.shadow-analysis',
      label: 'Shadow analysis',
      aidKind: 'policy',
      read: {
        id: 'warden.truths.read',
        label: 'Rule catalog',
        path: '/truths',
      },
      authorityBoundary: 'Shadow analysis informs rule publication but does not publish rules by itself.',
    },
  ],
  artifacts: [
    {
      id: 'warden.audit-pack',
      label: 'Audit pack',
      resolverScheme: 'warden://',
      read: {
        id: 'warden.audit-pack.read',
        label: 'Gate verdict',
        path: '/v1/demo/acquisition-assets/shared-identity-core/gates/dd-evidence.identity-data-residency',
      },
      requiredProvenance: ['rule_id', 'evidence_refs', 'verdict'],
    },
  ],
  safety: commonSafety,
}

export const marqueeConsoleAdapters = [
  quorumConsoleAdapter,
  atlasConsoleAdapter,
  wardenConsoleAdapter,
] satisfies ConsoleAdapter[]

function command(
  id: string,
  label: string,
  method: ConsoleCommandDescriptor['request']['method'],
  path: string,
  authority: ConsoleCommandDescriptor['authority'],
  fields: ConsoleFieldDescriptor[] = [],
  body?: Record<string, unknown>,
): ConsoleCommandDescriptor {
  return {
    id,
    label,
    authority,
    request: {
      method,
      path,
      body: body ?? bodyFromFields(fields),
    },
    form: fields.length ? { fields } : undefined,
  }
}

function bodyFromFields(fields: ConsoleFieldDescriptor[]): Record<string, unknown> | undefined {
  const bodyFields = fields.filter((field) => !field.id.endsWith('_id') && field.id !== 'id')
  if (!bodyFields.length) return undefined
  return Object.fromEntries(bodyFields.map((field) => [field.id, `{${field.id}}`]))
}
