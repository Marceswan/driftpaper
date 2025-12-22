# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

This is a Salesforce DX project called `flowRecordDisplay` that provides a Lightning Web Component (LWC) for displaying and editing records using FlexiPage layouts. The component can be used in Salesforce Flows and on record pages.

## Key Commands

### Development Commands

```bash
# Lint the code (LWC and Aura components)
npm run lint

# Run unit tests
npm run test
npm run test:unit

# Run tests in watch mode
npm run test:unit:watch

# Run tests with coverage
npm run test:unit:coverage

# Format code with Prettier
npm run prettier

# Check code formatting
npm run prettier:verify
```

### Salesforce Deployment Commands

```bash
# Deploy to org (deploy only the files being worked on)
sf project deploy start --source-path force-app/main/default/lwc/flexipageRecordForm
sf project deploy start --source-path force-app/main/default/classes/FlexiPageToolingService.cls

# Deploy Apex classes with their metadata
sf project deploy start --source-path force-app/main/default/classes/FlexiPageToolingService.cls --source-path force-app/main/default/classes/FlexiPageToolingService.cls-meta.xml

# Run Apex tests after deployment
sf apex run test --test-name-match FlexiPageToolingServiceTest --code-coverage --result-format human
```

## Architecture Overview

### Core Components

1. **flexipageRecordForm LWC** (`force-app/main/default/lwc/flexipageRecordForm/`)
   - Main component that renders Salesforce records using FlexiPage layouts
   - Supports both read-only and edit modes
   - Can be used in Flows with the `flowContext` attribute
   - Handles field visibility rules and dynamic rendering

2. **FlexiPageToolingService** (`force-app/main/default/classes/FlexiPageToolingService.cls`)
   - Apex service that retrieves FlexiPage metadata using Tooling API
   - Uses Named Credential `Tooling_API_Credential` for authentication
   - Methods:
     - `getFlexiPageMetadata()`: Fetches FlexiPage layout configuration
     - `getFieldValues()`: Retrieves field values for a record

3. **FlexiPageToolingServiceTest** (`force-app/main/default/classes/FlexiPageToolingServiceTest.cls`)
   - Test class for the Apex service
   - Minimum required code coverage: 90%

### Key Features

- **Dynamic Layout Rendering**: Uses Salesforce FlexiPage metadata to dynamically render forms
- **Field Visibility Rules**: Supports complex visibility rules with boolean filters
- **Flow Integration**: Can be used in Salesforce Flows to display/edit records
- **Default Values**: Supports setting default field values via component attributes
- **Field Exclusion**: Allows excluding specific fields from the layout

### Important Implementation Details

- The component uses lowercase field names internally for consistency
- Visibility rules support operators: CONTAINS, EQUAL, NE, GT, GE, LE, LT
- Default values can be passed as comma or semicolon-separated field:value pairs
- The component handles both record creation and updates
- Built-in excluded fields: CreatedById, LastModifiedById, Id

### Testing Approach

- Jest is configured for LWC unit testing
- Use `sfdx-lwc-jest` for running tests
- Apex tests use the FlexiPageToolingServiceTest class
- Always run tests after making changes to ensure 90%+ coverage

## Custom Property Editor (CPE) Development Guide

Reference: https://gist.github.com/Marceswan/82e9cc22b43eb695300fb0d5dc2add8e

### Overview

A Custom Property Editor (CPE) is a specialized Lightning Web Component that provides a configuration interface for Flow screen components within Salesforce Flow Builder. CPEs enable complex, dynamic configuration scenarios beyond standard property editors.

### Key Architecture Components

1. **Main CPE Component** - The primary LWC that implements the property editor interface
2. **External Helper Components** - Additional LWCs for complex UI elements
3. **Apex Support Classes** - Server-side logic for metadata retrieval and processing

### Core JavaScript Properties and Methods

```javascript
// Essential CPE properties
@api
get builderContext() {
    return this._builderContext;
}
set builderContext(value) {
    this._builderContext = value;
    // React to context changes
}

@api
get inputVariables() {
    return this._inputVariables;
}
set inputVariables(value) {
    this._inputVariables = value;
    // Process configuration values
}

// Dispatch configuration changes
dispatchConfigurationChange(name, value) {
    const valueChangeEvent = new CustomEvent('configuration_editor_input_value_changed', {
        bubbles: true,
        cancelable: false,
        composed: true,
        detail: {
            name: name,
            newValue: value
        }
    });
    this.dispatchEvent(valueChangeEvent);
}
```

### Key Interfaces

- **`builderContext`**: Provides Flow metadata (variables, formulas, stages)
- **`inputVariables`**: Current configuration values
- **`genericTypeMappings`**: Handles sObject type mappings
- **`automaticOutputVariables`**: Access to output variables

### Implementation Best Practices

1. **Error Handling**: Implement comprehensive try-catch blocks and user-friendly error messages
2. **Loading States**: Show spinners during async operations
3. **Immediate Event Dispatch**: Dispatch configuration changes immediately on user interaction
4. **Value Preservation**: Maintain valid selections even when options change
5. **Flow Variable Support**: Support both direct values and Flow variable references

### Advanced Features

- **Progressive Disclosure**: Show/hide options based on selections
- **Dynamic Metadata Loading**: Fetch Salesforce metadata on-demand
- **Complex Validation**: Implement multi-field validation rules
- **Performance Optimization**: Debounce API calls and cache results
- **External Service Integration**: Connect to external systems for configuration data

### Example: Dynamic Picklist Implementation

```javascript
handleFieldSelection(event) {
    const selectedField = event.detail.value;
    
    // Dispatch the change immediately
    this.dispatchConfigurationChange('selectedField', selectedField);
    
    // Load dependent options
    this.loadDependentOptions(selectedField);
}

async loadDependentOptions(fieldName) {
    this.isLoading = true;
    try {
        const options = await getFieldOptions({ fieldName });
        this.dependentOptions = options;
    } catch (error) {
        this.handleError(error);
    } finally {
        this.isLoading = false;
    }
}
```

### Common CPE Patterns

1. **Metadata-Driven Configuration**: Load object/field metadata dynamically
2. **Conditional Visibility**: Show/hide inputs based on other selections
3. **Multi-Value Inputs**: Handle arrays and complex data structures
4. **Preview Components**: Show live previews of configuration
5. **Validation Feedback**: Real-time validation with clear error messages

## Tooling API to Metadata API Migration Plan

### Project Overview

This section documents the migration from Tooling API to Metadata API for FlexiPage metadata retrieval. The migration aims to improve performance, eliminate API callout limits, and simplify the architecture.

### Current State (Tooling API)
- **Service Class**: `FlexiPageToolingService.cls`
- **Authentication**: Named Credential `Tooling_API_Credential`
- **Method**: HTTP callouts to `/services/data/v60.0/tooling/query/`
- **Limitations**: API limits, network latency, requires Named Credential setup

### Target State (Metadata API)
- **Service Class**: `FlexiPageMetadataService.cls` (new)
- **Authentication**: Direct Apex access (no Named Credential needed)
- **Method**: `Metadata.Operations.retrieve()`
- **Benefits**: No API limits, better performance, simpler setup

### Migration Phases

#### Phase 1: Create New Service (Week 1)
1. Create `FlexiPageMetadataService.cls` with Metadata API implementation
2. Implement core methods:
   - `getFlexiPageMetadata()` using Metadata.Operations
   - Error handling for metadata retrieval
   - Response formatting to match existing structure

#### Phase 2: Testing Infrastructure (Week 1)
1. Create `FlexiPageMetadataServiceTest.cls` with 90%+ coverage
2. Create comparison tests between old and new implementations
3. Performance benchmarking tests
4. Edge case testing (missing metadata, permissions)

#### Phase 3: LWC Compatibility Layer (Week 2)
1. Add feature toggle in Custom Settings/Metadata
2. Update `flexipageRecordForm.js` to support both APIs
3. Create adapter pattern for response differences
4. Test with various FlexiPage configurations

#### Phase 4: Gradual Migration (Week 2)
1. Enable new API in sandbox environments
2. A/B testing with selected users
3. Monitor performance metrics
4. Address any compatibility issues

#### Phase 5: Cleanup (Week 3)
1. Remove Tooling API implementation
2. Delete Named Credential
3. Update all documentation
4. Final production deployment

### Technical Implementation Details

#### New Metadata API Service Structure
```apex
public class FlexiPageMetadataService {
    public static String getFlexiPageMetadata(String developerName) {
        Metadata.DeployContainer container = new Metadata.DeployContainer();
        List<String> flexiPageNames = new List<String>{'FlexiPage.' + developerName};
        
        List<Metadata.Metadata> components = 
            Metadata.Operations.retrieve(Metadata.MetadataType.FlexiPage, flexiPageNames);
        
        // Process and return metadata
    }
}
```

#### Feature Toggle Implementation
```apex
public static Boolean useMetadataAPI() {
    FlexiPage_Migration_Settings__c settings = 
        FlexiPage_Migration_Settings__c.getOrgDefaults();
    return settings.Use_Metadata_API__c ?? false;
}
```

### Migration Checklist

- [ ] Create new Metadata API service class
- [ ] Implement comprehensive test coverage
- [ ] Add feature toggle mechanism
- [ ] Update LWC for dual API support
- [ ] Performance testing and comparison
- [ ] Sandbox testing and validation
- [ ] Production deployment plan
- [ ] Remove legacy Tooling API code
- [ ] Update documentation

### Risk Mitigation

1. **Backward Compatibility**: Maintain both implementations during transition
2. **Permission Handling**: Test with various user profiles
3. **Response Format**: Create adapters for any format differences
4. **Rollback Plan**: Keep feature toggle for quick reversion

### Performance Expectations

- **Current (Tooling API)**: ~500-1000ms per request
- **Target (Metadata API)**: ~100-300ms per request
- **API Limit Impact**: From counting against limits to zero impact

### Project Memory Reference

Knowledge Graph Entities:
- **Project**: FlexiPage Tooling to Metadata API Migration
- **Current Implementation**: Tooling API approach details
- **Target Implementation**: Metadata API approach details
- **Risks**: Identified migration risks and mitigations