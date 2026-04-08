<script lang="ts">
	import * as Card from '$lib/components/ui/card/index.js';
	import { Badge } from '$lib/components/ui/badge/index.js';
	import { Button } from '$lib/components/ui/button/index.js';

	const featureNames: Record<string, string> = {
		compliance: 'Compliance Modes',
		audit_trail: 'Audit Trail',
		sso: 'Single Sign-On',
		story_generation: 'Story Generation',
		advanced_analytics: 'Advanced Analytics',
		multi_org: 'Multi-Organization',
		encryption_at_rest: 'Encryption at Rest',
		full_policy_engine: 'Full Policy Engine',
		advanced_redaction: 'Advanced Redaction'
	};

	const featureDescriptions: Record<string, string> = {
		compliance: 'SOX, PCI-DSS, and SR 11-7 compliance modes with chain verification and signing.',
		audit_trail: 'Full audit trail with tamper-evident logging and detailed activity tracking.',
		sso: 'SAML and OIDC single sign-on integration for your identity provider.',
		story_generation: 'AI-powered story generation from code context and git history.',
		advanced_analytics: 'Cost analytics, usage breakdowns, and trend reporting across your organization.',
		multi_org: 'Manage multiple organizations with isolated data and shared billing.',
		encryption_at_rest: 'AES-256 encryption for all stored traces and artifacts.',
		full_policy_engine: 'Granular policy rules with custom conditions and automated enforcement.',
		advanced_redaction: 'Pattern-based and ML-powered redaction of sensitive data in traces.'
	};

	let { feature, title }: { feature: string; title?: string } = $props();

	const displayName = $derived(title || featureNames[feature] || feature);
	const description = $derived(featureDescriptions[feature] || 'This feature is available in TraceVault Enterprise.');
</script>

<Card.Root class="border-dashed">
	<Card.Header>
		<div class="flex items-center gap-2">
			<Badge variant="secondary">Enterprise</Badge>
			<Card.Title>{displayName}</Card.Title>
		</div>
	</Card.Header>
	<Card.Content>
		<p class="text-sm text-muted-foreground mb-4">{description}</p>
		<Button variant="outline" size="sm" href="https://tracevault.dev/compare">
			Learn more
		</Button>
	</Card.Content>
</Card.Root>
