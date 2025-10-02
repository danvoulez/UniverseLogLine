# Multi-Tenant LogLine ID Onboarding System

A comprehensive onboarding system that adapts to different tenant configurations, from individual users to enterprise organizations with custom branding and policies.

## 🏢 Multi-Tenancy Overview

LogLineOS supports multiple deployment modes:

### **Single-Tenant Mode**
- **Direct LogLine ID use** - Users create personal accounts
- **Simple onboarding** - Welcome → Passkey → Identity → Complete
- **No organization context** - Pure individual identity

### **Multi-Tenant Mode**
- **Organization-specific onboarding** - Custom branding and policies
- **Invitation-based joining** - Users join via invitation links
- **Role-based setup** - Different flows for members vs. admins
- **Custom policies** - Organizations can require/disable features

## 🎯 Onboarding Flow Variations

### **Scenario 1: Individual User (Single-Tenant)**
```
1. Tenant Selection → [Choose "Personal Use"]
2. Welcome → Standard LogLine ID introduction
3. Passkey Setup → Optional (user choice)
4. Identity Creation → Create personal identity
5. Completion → Ready to use LogLine ID
```

### **Scenario 2: Organization Member (Invitation Link)**
```
Auto-detected tenant from invitation →
1. Welcome → Custom organization branding
2. Passkey Setup → Required (if org policy demands)
3. Identity Creation → Organization profile completion
4. Completion → Welcome to [Organization Name]
```

### **Scenario 3: Organization Search**
```
1. Tenant Selection → [Search for organization]
2. Organization Selection → Choose from search results
3. Welcome → Organization-branded introduction
4. Passkey Setup → Per organization policy
5. Identity Creation → Organization member profile
6. Completion → Organization-specific next steps
```

## 🔍 Tenant Detection Methods

### **1. URL Subdomain**
```
acme.loglineid.com → Auto-detects Acme Corporation
startup.loglineid.com → Auto-detects StartupCo
```

### **2. URL Parameters**
```
loglineid.com?tenant=acme → Loads Acme Corporation context
loglineid.com?org=startup → Loads StartupCo context
```

### **3. Invitation Links**
```
loglineid.com?invite=xyz123 → Decodes invitation token
→ Organization: Acme Corporation
→ Inviter: John Doe
→ Role: Admin
→ Expires: 7 days
```

### **4. Manual Search**
Users can search for organizations by:
- Company name ("Acme Corporation")
- Domain name ("acme.com")
- Organization ID ("acme")

## 🎨 Tenant Customization

### **Branding Configuration**
```typescript
interface TenantConfig {
  id: 'acme';
  name: 'Acme Corporation';
  branding: {
    companyName: 'Acme Corporation';
    logoUrl: '/logos/acme-logo.svg';
    colors: {
      primary: '#E74C3C';     // Main brand color
      secondary: '#C0392B';   // Secondary brand color
      accent: '#F39C12';      // Accent color
    };
    customCSS?: string;       // Custom CSS overrides
  };
}
```

### **Policy Configuration**
```typescript
interface TenantPolicies {
  requireBiometric: true;        // Force passkey setup
  allowSkipIdentity: false;      // Require identity completion
  customTermsUrl: 'https://acme.com/terms';
  customPrivacyUrl: 'https://acme.com/privacy';
}
```

### **Feature Configuration**
```typescript
interface TenantFeatures {
  ghostIdentities: true;         // Enable ghost identities
  organizationRoles: true;       // Enable role management
  advancedSecurity: true;        // Enable advanced features
}
```

## 🚀 Implementation Guide

### **Basic Multi-Tenant Setup**
```tsx
import { MultiTenantOnboardingFlow } from './onboarding';

function App() {
  const [showOnboarding, setShowOnboarding] = useState(false);

  const handleComplete = (context: OnboardingContext) => {
    if (context.tenant) {
      // Redirect to organization dashboard
      window.location.href = `/org/${context.tenant.id}/dashboard`;
    } else {
      // Redirect to personal dashboard
      window.location.href = '/dashboard';
    }
  };

  return (
    <MultiTenantOnboardingFlow
      isVisible={showOnboarding}
      onComplete={handleComplete}
      autoDetectTenant={true}
    />
  );
}
```

### **Custom Tenant Detection**
```tsx
import { TenantDetectionService } from './onboarding';

// Manual tenant detection
const detection = await TenantDetectionService.detectTenant();
console.log(detection);
// {
//   detectionMethod: 'invitation-link',
//   tenant: { id: 'acme', name: 'Acme Corporation', ... },
//   invitationData: { token: 'xyz123', role: 'admin', ... }
// }

// Generate invitation links
const inviteLink = TenantDetectionService.generateInviteLink(
  'acme',           // Tenant ID
  'John Doe',       // Inviter name
  'admin'           // Role
);
// Returns: https://loglineid.com?invite=eyJvcmciOiJhY21lIiw...
```

## 🔐 Security Considerations

### **Invitation Token Security**
- **JWT-based tokens** (in production) with signature verification
- **Expiration times** - Default 7 days, configurable per organization
- **Role validation** - Tokens specify invited user's role
- **One-time use** - Tokens invalidated after successful onboarding

### **Tenant Isolation**
- **Data segregation** - Each tenant's data is isolated
- **Policy enforcement** - Tenant policies are strictly enforced
- **Branding security** - Custom CSS is sanitized and scoped

### **Cross-Tenant Prevention**
- **Domain validation** - Subdomains must match registered tenants
- **Invitation verification** - Tokens are cryptographically verified
- **Session isolation** - Tenant context is session-scoped

## 📊 Analytics and Monitoring

### **Onboarding Metrics**
```typescript
// Track completion rates by tenant
const metrics = {
  tenantId: 'acme',
  completionRate: 0.85,        // 85% complete onboarding
  averageTime: 180,            // 3 minutes average
  stepDropoff: {
    'tenant-selection': 0.05,  // 5% drop off
    'passkey-setup': 0.15,     // 15% drop off
    'identity-creation': 0.10,  // 10% drop off
  },
  invitationSuccess: 0.92      // 92% invitation acceptance
};
```

### **Common Integration Points**
```typescript
// Post-onboarding hooks
onComplete: (context: OnboardingContext) => {
  // Analytics tracking
  analytics.track('onboarding_completed', {
    tenantId: context.tenant?.id,
    userType: context.userType,
    mode: context.mode,
    hasInvitation: !!context.invitationToken
  });
  
  // User provisioning
  if (context.tenant) {
    provisionOrganizationUser(context);
  } else {
    provisionIndividualUser(context);
  }
  
  // Navigation
  navigateToAppropriateDestination(context);
}
```

## 🧪 Testing Scenarios

### **Test URLs for Development**
```bash
# Individual user onboarding
http://localhost:3000

# Acme Corporation (subdomain)
http://acme.localhost:3000

# StartupCo (URL parameter)
http://localhost:3000?tenant=startup

# Invitation link (Acme admin)
http://localhost:3000?invite=eyJvcmciOiJhY21lIiwiaW52aXRlciI6IkpvaG4gRG9lIiwicm9sZSI6ImFkbWluIn0=

# Invitation link (StartupCo member)
http://localhost:3000?invite=eyJvcmciOiJzdGFydHVwIiwiaW52aXRlciI6IkphbmUgU21pdGgiLCJyb2xlIjoibWVtYmVyIn0=
```

### **Test Tenant Configurations**
The system includes mock tenant data for development:

**Acme Corporation:**
- Requires biometric authentication
- Custom red/orange branding
- Full feature set enabled
- Strict policies (no skipping steps)

**StartupCo:**
- Optional biometric authentication
- Purple branding
- Basic feature set
- Flexible policies (allows skipping)

## 🔮 Advanced Features

### **Dynamic Step Configuration**
Different tenants can have different onboarding flows:
```typescript
// Enterprise tenant - full security flow
const enterpriseSteps = [
  'tenant-selection',
  'welcome',
  'security-briefing',    // Custom step
  'passkey-setup',
  'identity-creation',
  'role-assignment',      // Custom step
  'completion'
];

// Startup tenant - streamlined flow
const startupSteps = [
  'tenant-selection',
  'welcome',
  'passkey-setup',
  'completion'            // Skip identity for faster onboarding
];
```

### **Custom Step Components**
Organizations can inject custom onboarding steps:
```typescript
const customSteps = {
  'security-briefing': SecurityBriefingStep,
  'role-assignment': RoleAssignmentStep,
  'compliance-agreement': ComplianceStep
};
```

### **White-Label Deployments**
For enterprise customers who want complete customization:
```typescript
const whiteLabel = {
  productName: 'AcmeID',           // Instead of "LogLine ID"
  customDomain: 'id.acme.com',     // Custom domain
  fullBranding: true,              // Complete UI customization
  hideLogLineBranding: true        // Remove LogLine references
};
```

---

This multi-tenant onboarding system provides the flexibility needed for LogLineOS to serve both individual users and large organizations while maintaining security, usability, and brand consistency.