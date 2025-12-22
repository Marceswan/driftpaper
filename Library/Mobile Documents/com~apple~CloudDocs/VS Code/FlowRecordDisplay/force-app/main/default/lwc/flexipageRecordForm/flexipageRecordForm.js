/**
 * @description       :
 * @author            : Marc Swan
 * @group             :
 * @last modified on  : 07-19-2025
 * @last modified by  : ChangeMeIn@UserSettingsUnder.SFDoc
 * Modifications Log
 * Ver   Date         Author      Modification
 * 1.0   06-16-2024   Marc Swan   Initial Version
 **/
import { LightningElement, api, track, wire } from 'lwc';
import getFieldValues from '@salesforce/apex/FlexiPageToolingService.getFieldValues';
import getFlexiPageMetadata from '@salesforce/apex/FlexiPageToolingService.getFlexiPageMetadata';
import { parseFlexiPageJson } from './utils';
import { createRecord, updateRecord, getRecord } from "lightning/uiRecordApi";
import { ShowToastEvent } from "lightning/platformShowToastEvent";
import { FlowAttributeChangeEvent } from 'lightning/flowSupport';
import { getObjectInfo } from "lightning/uiObjectInfoApi";
import { NavigationMixin } from 'lightning/navigation';
import { refreshApex } from '@salesforce/apex';

// Define an array of field names that should be read-only (not editable)
const readOnlyFields = ['CreatedById', 'LastModifiedById', 'Id', 'SystemModstamp', 'CreatedDate', 'LastModifiedDate', 'OwnerId'];

export default class FlexipageRecordForm extends NavigationMixin(LightningElement) {
    // Public properties that can be set as attributes on the component
    @api recordId; // The ID of the record to be displayed or edited
    @api objectApiName; // The API name of the object associated with the record
    @api flexiPageName; // The developer name of the FlexiPage to be used for rendering the form
    @api altField; // An alternative field to be used for fetching record data
    @api isReadOnly = false; // A boolean indicating whether the form should be read-only or editable
    @api debugEnabled = false; // A boolean indicating whether debug mode should be enabled
    @api cardTitle = ''; // The title to be displayed on the card
    @api showIcon = false; // A boolean indicating whether to show the object icon
    @api varRecord; // A variable to store the record data, used in the context of a Flow
    @api flowContext; // A boolean indicating whether the component is being used within a Flow context
    @api cancelLabel = 'Cancel'; // The label for the cancel button
    @api saveLabel = 'Save'; // The label for the save button
    @api excludedFields = ''; // A comma-separated list of field API names to exclude from the layout
    @api defaultValues = '';
    @api fieldPageName = '';
    @api useFieldHistory = false;
    
    // Output-only properties for Flow compatibility
    @api enableVisibilityRules = false;
    @api columnLayout = '';
    @api customCssClass = '';
    @api fieldSetName = '';
    @api debugMode = false;
    @api showSectionHeaders = false;
    @api highlightRequiredFields = false;
    @api showEditButtons = false;
    @api enableCollapsibleSections = false;
    @api enableConditionalFormatting = false;

    get allExcludedFields() {
        // Only exclude user-specified fields, not system fields
        const excludedFieldsList = [];

        // Add any user-specified excluded fields if they exist
        if (this.excludedFields) {
            const userExcludedFields = this.excludedFields
                .split(',')
                .map(field => field.trim());
            excludedFieldsList.push(...userExcludedFields);
        }

        // Normalize all fields (convert to lowercase)
        return excludedFieldsList.map(field => field.toLowerCase());
    }

    // Tracked properties for managing the component's state
    @track sections = []; // An array to store the sections of the form
    @track isOpen = true; // A boolean indicating whether the form is open or closed
    @track editMode = false; // A boolean indicating whether the form is in edit mode or not
    @track iconUrl = ''; // The URL of the object icon
    @track objectIcon = ''; // The name of the object icon
    @track isLoading = false; // Loading state for the component

    // Internal properties for storing data
    error; // An object to store any errors that occur
    @track recordData = {}; // An object to store the record data
    @track fieldMetadata = {}; // An object to store field metadata (labels, types, etc.)
    config; // An object to store the FlexiPage configuration
    fields = []; // An array to store the field names
    modelType = null; // A variable to store the model type (not used in this code)
    refreshKey = 0; // Key to force re-render
    wiredFieldValuesResult; // Store the wire result for refreshApex
    fieldApiNamesForWire = []; // Fields to fetch via wire

    connectedCallback() {
        console.log('Starting connectedCallback');

        const { original: defaultValues, lowercase: lookupValues } = this.parseDefaultValues();
        const isNewRecord = !this.recordId || this.recordId.length < 15;

        // Only send original case values to the flow
        if (this.flowContext && Object.keys(defaultValues).length > 0) {
            console.log('Dispatching initial default values to flow:', defaultValues);
            this.varRecord = { ...defaultValues };
            this.dispatchEvent(
                new FlowAttributeChangeEvent('varRecord', this.varRecord)
            );
        }

        if (isNewRecord) {
            console.log('Handling new record creation scenario');
            this.editMode = true;
            // Use lowercase map for internal recordData
            this.recordData = { ...lookupValues };

            getFlexiPageMetadata({ developerName: this.flexiPageName })
                .then(result => {
                    try {
                        console.log('FlexiPage metadata received');
                        this.config = JSON.parse(result);

                        // Extract metadata from the response
                        let parsedSections = parseFlexiPageJson(this.config.metadata || this.config, this.recordData);
                        parsedSections = this.removeExcludedFields(parsedSections);
                        this.sections = this.processSections(parsedSections);
                        this.fields = this.collectFields(parsedSections);

                        setTimeout(() => {
                            this.applyDefaultValuesToForm(defaultValues);
                        }, 0);

                    } catch (e) {
                        console.error('Error processing FlexiPage config:', e);
                        this.error = e;
                    }
                })
                .catch(error => {
                    console.error('Error loading FlexiPage metadata:', error);
                    this.error = error;
                });
        } else {
            console.log('Loading existing record configuration');
            this.loadFlexiPageConfig();
        }
    }

    @wire(getObjectInfo, {objectApiName: "$objectApiName"})
    handleResult({error, data}) {
        // This method is a wire adapter that retrieves the object information
        if (data) {
            const objectInformation = data;
            const iconUrl = objectInformation.themeInfo.iconUrl;

            // If showIcon is true and iconUrl is not empty
            if (this.showIcon && iconUrl && iconUrl.trim() !== '') {
                const urlList = iconUrl.split('/');
                if (urlList.length > 2) {
                    const iconSvg = urlList[urlList.length - 1];
                    const iconName = iconSvg.substring(0, iconSvg.lastIndexOf('_'));
                    this.objectIcon = `${urlList[urlList.length - 2]}:${iconName}`; // Set the object icon
                }
            }
        } else if (error) {
            console.error('Error fetching object info:', error);
        }
    }

    loadFlexiPageConfig() {
        // This method loads the FlexiPage configuration
        console.log('loadFlexiPageConfig called');

        getFlexiPageMetadata({developerName: this.flexiPageName})
            .then(result => {
                console.log('FlexiPage config loaded:', result);
                try {
                    this.config = JSON.parse(result); // Parse the FlexiPage configuration JSON
                    this.fetchFieldValues(); // Fetch the field values
                } catch (e) {
                    console.error('Error parsing JSON:', e);
                    console.error('Received JSON:', result);
                    this.error = e; // Store the error
                }
            })
            .catch(error => {
                console.error('Error loading flexipage config:', error);
                this.error = error; // Store the error
            });
    }

    // Wire adapter to fetch field values
    @wire(getFieldValues, {
        recordId: '$recordId',
        objectApiName: '$objectApiName',
        fieldApiNames: '$fieldApiNamesForWire'
    })
    wiredFieldValues(result) {
        this.wiredFieldValuesResult = result;
        const { data, error } = result;
        
        if (data) {
            console.log('Wire: Received field values from server:', data);
            // New format includes values and metadata
            this.recordData = this.mapFieldValues(data.values || data);
            this.fieldMetadata = data.metadata || {};

            console.log('Wire: Updated recordData:', this.recordData);
            console.log('Wire: Updated fieldMetadata:', this.fieldMetadata);
            
            // Debug OwnerId specifically
            if (this.fieldMetadata.ownerid) {
                console.log('Wire: OwnerId metadata:', this.fieldMetadata.ownerid);
                console.log('Wire: OwnerId referenceNameValue:', this.fieldMetadata.ownerid.referenceNameValue);
            }

            // Only process sections if we have config
            if (this.config) {
                this.processFieldData();
            }
        } else if (error) {
            console.error('Wire: Error fetching field values:', error);
            this.error = error;
            this.isLoading = false;
        }
    }
    
    fetchFieldValues() {
        console.log('==== Starting fetchFieldValues ====');
        console.log('Current defaultValues property:', this.defaultValues);
        console.log('ObjectApiName:', this.objectApiName);
        console.log('RecordId:', this.recordId);

        // First, parse the FlexiPage to get all fields
        let parsedSections = parseFlexiPageJson(this.config.metadata || this.config, {});
        this.fields = this.collectFields(parsedSections);
        console.log('Collected fields from FlexiPage:', this.fields);

        const excludedFields = this.allExcludedFields;
        const filteredFields = this.fields.filter(field =>
            !excludedFields.includes(field.toLowerCase())
        );
        console.log('Filtered fields for getFieldValues:', filteredFields);

        // Show loading state
        this.isLoading = true;
        
        // Update the fields for the wire adapter - this will trigger wire to fetch data
        this.fieldApiNamesForWire = [...filteredFields];
    }
    
    processFieldData() {
        console.log('==== Starting section processing ====');
        // Force complete re-render by creating new sections array
        let parsedSections = parseFlexiPageJson(this.config.metadata || this.config, this.recordData);

        console.log('==== Applying default values ====');
        this.applyDefaultValues(parsedSections);

        parsedSections = this.removeExcludedFields(parsedSections);
        this.calculateVisibility(parsedSections);
        
        // Process new sections
        const newSections = this.processSections(parsedSections);
        this.fields = this.collectFields(parsedSections);
        
        // Force complete UI update using microtask
        this.sections = [];
        this.refreshKey++;
        
        // Use microtask to ensure DOM processes the empty sections
        Promise.resolve().then(() => {
            // Then set the new sections
            this.sections = newSections;
            
            console.log('==== Finished processing sections ====');
            console.log('Final sections:', this.sections);
            console.log('Updated recordData:', this.recordData);
            console.log('Refresh key:', this.refreshKey);
            
            // Another microtask to ensure render completion
            Promise.resolve().then(() => {
                this.isLoading = false;
                this.checkAltFieldAndRefetch();
            });
        });
    }

    // Added this method to apply default values to the parsed sections
    applyDefaultValues(parsedSections) {
        console.log('Starting to apply default values');

        const defaultValues = this.parseDefaultValues();
        if (Object.keys(defaultValues).length === 0) {
            console.log('No default values to apply');
            return;
        }

        const isNewRecord = !this.recordId || this.recordId.length < 15;
        console.log('Available default values:', defaultValues);

        let updatedFields = false;

        Object.entries(parsedSections).forEach(([sectionId, section]) => {
            Object.entries(section.columns).forEach(([columnId, column]) => {
                Object.entries(column.fields).forEach(([fieldId, field]) => {
                    if (defaultValues.hasOwnProperty(fieldId) &&
                        (isNewRecord || !field.value)) {
                        field.value = defaultValues[fieldId];

                        // Update recordData to match
                        this.recordData[fieldId.toLowerCase()] = defaultValues[fieldId];
                        updatedFields = true;

                        console.log(`Applied default value to ${fieldId}: ${field.value}`);
                    }
                });
            });
        });

        // If we're in a flow context and we updated any fields, update varRecord
        if (this.flowContext && updatedFields) {
            this.varRecord = { ...this.recordData };
            this.dispatchEvent(new FlowAttributeChangeEvent('varRecord', this.varRecord));
        }
    }

    // A simple method that removes excluded fields from the parsed sections
    removeExcludedFields(parsedSections) {
        if (!parsedSections) return {};

        // Get excluded fields from our single source of truth
        const excludedFields = this.allExcludedFields;
        console.log('Removing excluded fields:', excludedFields);

        // Create a deep copy
        const sections = JSON.parse(JSON.stringify(parsedSections));

        // Track what we're removing for debugging
        let removedFields = [];

        Object.values(sections).forEach(section => {
            Object.values(section.columns).forEach(column => {
                Object.keys(column.fields).forEach(fieldId => {
                    if (excludedFields.includes(fieldId.toLowerCase())) {
                        delete column.fields[fieldId];
                        removedFields.push(fieldId);
                    }
                });
            });
        });

        console.log('Removed fields:', removedFields);
        return sections;
    }

    mapFieldValues(data) {
        // This method maps the field values to a normalized object
        let mappedValues = {};
        for (const key in data) {
            if (data.hasOwnProperty(key)) {
                const normalizedKey = key.toLowerCase();
                mappedValues[normalizedKey] = data[key];
            }
        }
        return mappedValues;
    }

    checkAltFieldAndRefetch() {
        // This method checks if an alternative field is provided and re-fetches field values if necessary
        const altFieldValue = this.recordData[this.altField?.toLowerCase()];
        if (altFieldValue) {
            console.log(`Re-fetching field values using altField value: ${altFieldValue}`);
            this.recordId = altFieldValue;
            this.fetchFieldValues(); // Re-fetch field values using the alternative field value
        }
    }

    calculateVisibility(parsedSections) {
        // This method calculates the visibility of fields based on visibility rules
        Object.keys(parsedSections).forEach(sectionKey => {
            const section = parsedSections[sectionKey];
            Object.keys(section.columns).forEach(columnKey => {
                const column = section.columns[columnKey];
                Object.keys(column.fields).forEach(fieldKey => {
                    const field = column.fields[fieldKey];
                    if (field.visibilityRule) {
                        field.isVisible = this.evaluateVisibilityRule(field.visibilityRule); // Evaluate the visibility rule
                    }
                    console.log(`Field: ${fieldKey}, Value: ${field.value}, isVisible: ${field.isVisible}, isRequired: ${field.isRequired}, visibilityRule: ${JSON.stringify(field.visibilityRule)}`);
                });
            });
        });
    }

    evaluateVisibilityRule(visibilityRule) {
        // This method evaluates the visibility rule for a field
        console.log('evaluateVisibilityRule called with rule:', visibilityRule);
        if (!visibilityRule || !visibilityRule.criteria) {
            return true; // If no visibility rule or criteria, consider the field visible
        }

        const results = visibilityRule.criteria.map(criterion => {
            const leftFieldApiName = criterion.leftValue.replace('{!Record.', '').replace('}', '').toLowerCase();
            const leftValue = this.recordData[leftFieldApiName];
            const rightValue = criterion.rightValue;

            let conditionMet = false;

            switch (criterion.operator) {
                case 'CONTAINS':
                    conditionMet = leftValue?.includes(rightValue);
                    break;
                case 'EQUAL':
                    conditionMet = leftValue === rightValue; //EQUAL TO
                    break;
                case 'NE':
                    conditionMet = leftValue !== rightValue; //NOT EQUAL TO
                    break;
                case 'GT':
                    conditionMet = leftValue > rightValue; //GREATER THAN
                    break;
                case 'GE':
                    conditionMet = leftValue >= rightValue; //GREATER THAN OR EQUAL TO
                    break;
                case 'LE':
                    conditionMet = leftValue <= rightValue; //LESS THAN OR EQUAL TO
                    break;
                case 'LT':
                    conditionMet = leftValue < rightValue; //LESS THAN
                    break;
                default:
                    conditionMet = false;
            }

            console.log(`Criterion: ${JSON.stringify(criterion)}, leftFieldApiName: ${leftFieldApiName}, leftValue: ${leftValue}, rightValue: ${rightValue}, conditionMet: ${conditionMet}`);
            return conditionMet;
        });

        let isVisible = results.length > 0 ? results[0] : true; // If no results, consider the field visible

        if (visibilityRule.booleanFilter) {
            let expression = visibilityRule.booleanFilter
                .toLowerCase()
                .replace(/\band\b/g, '&&') // Replace 'AND' with '&&'
                .replace(/\bor\b/g, '||'); // Replace 'OR' with '||'

            results.forEach((result, index) => {
                expression = expression.replace(new RegExp(`\\b${index + 1}\\b`, 'g'), result);
            });

            try {
                isVisible = eval(expression); // Evaluate the boolean filter expression
            } catch (error) {
                console.error('Error evaluating boolean filter expression:', error);
                isVisible = false; // If an error occurs, consider the field not visible
            }
        }

        console.log('Visibility rule evaluated, isVisible:', isVisible);
        return isVisible;
    }

    // Add this helper method inside your FlexipageRecordForm class
    sanitizeHeader(header) {
        if (!header) return '';

        // Handle the system information pattern
        if (header.startsWith('@@@SFDC') && header.endsWith('SFDC@@@')) {
            header = header.replace('@@@SFDC', '').replace('SFDC@@@', '');
        }

        // Replace underscores with spaces
        header = header.replace(/_/g, ' ');

        // Add spaces between camelCase words
        header = header.replace(/([a-z])([A-Z])/g, '$1 $2');

        // Capitalize first letter of each word
        return header.split(' ')
            .filter(word => word.length > 0)
            .map(word => word.charAt(0).toUpperCase() + word.slice(1).toLowerCase())
            .join(' ');
    }

    processSections(parsedSections) {
        console.log('Processing sections with updated data');
        return Object.keys(parsedSections).map(sectionFacetId => {
            const section = parsedSections[sectionFacetId];

            // Process columns, only caring about visibility
            const columnsArray = Object.keys(section.columns)
                .map(columnId => {
                    const column = section.columns[columnId];

                    // Only include visible fields
                    const visibleFields = Object.entries(column.fields)
                        .filter(([_, field]) => field.isVisible);

                    // Build enhanced field data with metadata
                    const enhancedFields = visibleFields.map(([fieldId, field]) => {
                        const metadata = this.fieldMetadata[fieldId.toLowerCase()] || {};
                        const fieldValue = this.recordData[fieldId.toLowerCase()];
                        
                        console.log(`Processing field ${fieldId}:`);
                        console.log(`  - value: ${fieldValue}`);
                        console.log(`  - metadata:`, metadata);
                        console.log(`  - referenceNameValue: ${metadata.referenceNameValue}`);
                        
                        // Special debug for OwnerId
                        if (fieldId.toLowerCase() === 'ownerid') {
                            console.log('OWNER FIELD DEBUG:');
                            console.log('  - fieldId:', fieldId);
                            console.log('  - fieldValue:', fieldValue);
                            console.log('  - metadata:', JSON.stringify(metadata));
                            console.log('  - isReference:', metadata.type === 'REFERENCE');
                            console.log('  - referenceNameValue:', metadata.referenceNameValue);
                        }
                        
                        // Check if field is editable (not in read-only fields list)
                        const isEditable = !readOnlyFields.includes(fieldId);
                        
                        // Handle display value for reference fields
                        let displayValue = fieldValue;
                        if (metadata.type === 'REFERENCE' && metadata.referenceNameValue) {
                            displayValue = metadata.referenceNameValue;
                        }
                        
                        return {
                            fieldId: fieldId,
                            fieldData: {
                                ...field,
                                value: fieldValue,
                                displayValue: displayValue,
                                label: metadata.label || this.sanitizeHeader(fieldId),
                                type: metadata.type,
                                isReference: metadata.type === 'REFERENCE',
                                isCheckbox: metadata.type === 'BOOLEAN',
                                referenceObjectName: metadata.referenceObjectName,
                                referenceNameValue: metadata.referenceNameValue,
                                recordUrl: fieldValue && metadata.referenceObjectName ? `/lightning/r/${metadata.referenceObjectName}/${fieldValue}/view` : null,
                                isNameField: metadata.isNameField,
                                isEditable: isEditable
                            }
                        };
                    });

                    return {
                        ...column,
                        fieldIds: visibleFields.map(([id]) => id),
                        fields: Object.fromEntries(visibleFields),
                        enhancedFields: enhancedFields,
                        class: 'slds-col slds-size_1-of-2'
                    };
                })
                .filter(column => column.fieldIds.length > 0);

            return {
                sectionName: this.sanitizeHeader(section.label),
                sectionId: sectionFacetId,
                columns: columnsArray,
                isOpen: true,
                class: 'slds-section slds-is-open'
            };
        }).filter(section => section.columns.length > 0);
    }

    collectFields(parsedSections) {
        const excludedFields = this.allExcludedFields;
        let fields = [];

        Object.values(parsedSections).forEach(section => {
            Object.values(section.columns).forEach(column => {
                const sectionFields = Object.keys(column.fields)
                    .filter(fieldId => !excludedFields.includes(fieldId.toLowerCase()));
                fields.push(...sectionFields);
            });
        });

        return [...new Set(fields)];
    }

    get isDataAvailable() {
        // This getter returns a boolean indicating whether there are any sections available for rendering
        console.log('isDataAvailable called');
        return this.sections.length > 0;
    }
    
    get sectionsWithKey() {
        // Force re-render by creating new object references
        if (!this.sections || this.sections.length === 0) {
            return [];
        }
        
        // Deep clone sections to ensure new object references
        return this.sections.map(section => ({
            ...section,
            uniqueKey: `${section.sectionId}-${this.refreshKey}-${Date.now()}`,
            columns: section.columns.map(column => ({
                ...column,
                enhancedFields: column.enhancedFields ? column.enhancedFields.map(field => ({
                    ...field,
                    fieldData: {
                        ...field.fieldData,
                        // Force value update
                        value: field.fieldData.value,
                        displayValue: field.fieldData.value
                    }
                })) : [],
                fieldIds: [...(column.fieldIds || [])]
            }))
        }));
    }

    toggleSection(event) {
        // This method toggles the open/closed state of a section
        const sectionName = event.target.dataset.name; // Get the section name from the event target
        this.sections = this.sections.map(section => {
            if (section.sectionName === sectionName) {
                const isOpen = !section.isOpen; // Toggle the isOpen state
                return {
                    ...section,
                    isOpen,
                    class: `slds-section ${isOpen ? 'slds-is-open' : ''}` // Update the CSS class based on the isOpen state
                };
            }
            return section;
        });
    }

    handleEdit() {
        // This method sets the editMode to true, allowing the user to edit the record fields
        this.editMode = true;
    }

    handleCancel() {
        // This method sets the editMode to false, canceling the edit mode and reverting to read-only mode
        this.editMode = false;
    }

    handleSave() {
        // This method saves the changes made to the record fields
        const inputFields = this.template.querySelectorAll('lightning-input-field'); // Get all input fields
        const fields = {};
        const changedFields = {}; // Track the actual changed values

        if (inputFields) {
            inputFields.forEach(field => {
                // Only include changed, non-readonly fields that aren't excluded
                if (field.value !== field.defaultValue &&
                    !field.readOnly &&
                    !this.allExcludedFields.includes(field.fieldName.toLowerCase())) {
                    fields[field.fieldName] = field.value;
                    // Store the changed value for local update
                    changedFields[field.fieldName.toLowerCase()] = field.value;
                }
                field.reportValidity();
            });
        }

        console.log('Changed Fields:', JSON.stringify(fields));

        readOnlyFields.forEach(field => {
            delete fields[field]; // Remove read-only fields from save
        });

        const recordInput = this.recordId ? {fields: {...fields, Id: this.recordId}} : {
            apiName: this.objectApiName,
            fields
        };
        // Construct the recordInput object based on whether the record is being created or updated

        const saveOperation = this.recordId ? updateRecord(recordInput) : createRecord(recordInput);
        // Call the appropriate operation (updateRecord or createRecord) based on whether the record is being updated or created

        saveOperation
            .then((result) => {
                console.log('Save successful, result:', result);
                
                // Update recordId if this was a create operation
                if (!this.recordId && result.id) {
                    this.recordId = result.id;
                }
                
                // If flow context, update varRecord
                if (this.flowContext) {
                    this.varRecord = { ...this.varRecord, ...fields };
                    this.dispatchEvent(new FlowAttributeChangeEvent('varRecord', this.varRecord));
                }
                
                this.dispatchEvent(
                    new ShowToastEvent({
                        title: 'Success',
                        message: this.recordId ? 'Record updated successfully' : 'Record created successfully',
                        variant: 'success'
                    })
                );

                // Set editMode to false and immediately force a complete refresh
                this.editMode = false;
                
                // Force complete component refresh using refreshApex
                this.forceRefresh();
            })
            .catch(error => {
                this.dispatchEvent(
                    new ShowToastEvent({
                        title: 'Error saving record',
                        message: error.body.message, // Display the error message
                        variant: 'error'
                    })
                );
                console.error('Error saving record:', error);
            });
    }

    refreshData() {
        // This method refreshes the component's data after a successful save operation
        try {
            this.loadFlexiPageConfig(); // Load the FlexiPage configuration

            if (this.recordId && this.recordId.length >= 15) {
                this.fetchFieldValues(); // Fetch field values if a valid recordId is present
            } else {
                console.log('No valid recordId present, skipping fetchFieldValues');
            }

            console.log('refreshData completed successfully');
        } catch (error) {
            console.error('Error in refreshData:', error);
        }
    }

    handleFieldChange(event) {
        console.log('handleFieldChange event:', event);
        const fieldName = event.target.fieldName;
        const value = event.target.value;
        console.log('handleFieldChange', fieldName, value);

        // Update recordData
        if (fieldName) {
            this.recordData[fieldName.toLowerCase()] = value;
        }

        if (this.flowContext && fieldName) {
            const updatedRecord = { ...this.varRecord, [fieldName]: value };
            this.dispatchEvent(new FlowAttributeChangeEvent('varRecord', updatedRecord));
            console.log('updated record:', updatedRecord);
        }
    }

    parseDefaultValues() {
        console.log('Starting to parse default values string:', this.defaultValues);

        if (!this.defaultValues) {
            console.log('No default values provided');
            return { original: {}, lowercase: {} };
        }

        const result = {
            original: {},  // For flow updates - maintains original case
            lowercase: {}  // For internal lookups - everything lowercase
        };

        try {
            const separator = this.defaultValues.includes(';') ? ';' : ',';
            const fieldPairs = this.defaultValues.split(separator);
            console.log('Split field pairs:', fieldPairs);

            fieldPairs.forEach(pair => {
                if (!pair.trim()) return;

                const [fieldName, ...valueParts] = pair.split(':').map(item => item.trim());
                const value = valueParts.join(':');

                if (fieldName && value !== undefined) {
                    // Process value based on type
                    let processedValue = value;
                    if (value.toLowerCase() === 'true') processedValue = true;
                    else if (value.toLowerCase() === 'false') processedValue = false;
                    else if (!isNaN(value) && value.trim() !== '') processedValue = Number(value);

                    // Store original case for flow updates
                    result.original[fieldName] = processedValue;
                    // Store lowercase for internal lookups
                    result.lowercase[fieldName.toLowerCase()] = processedValue;

                    console.log(`Added default value mapping: ${fieldName} => ${processedValue}`);
                }
            });

        } catch (error) {
            console.error('Error parsing default values:', error);
        }

        return result;
    }

    applyDefaultValuesToForm(defaultValues) {
        if (!this.template) return;

        const form = this.template.querySelector('lightning-record-edit-form');
        if (!form) {
            console.log('Form not found, scheduling retry');
            setTimeout(() => this.applyDefaultValuesToForm(defaultValues), 100);
            return;
        }

        const inputFields = form.querySelectorAll('lightning-input-field');
        if (!inputFields?.length) {
            console.log('Input fields not found, scheduling retry');
            setTimeout(() => this.applyDefaultValuesToForm(defaultValues), 100);
            return;
        }

        console.log('Applying default values to form fields');
        let updatedValues = {};

        inputFields.forEach(field => {
            const fieldName = field.fieldName;
            // Use original case value if available
            const defaultValue = defaultValues[fieldName] || this.recordData[fieldName.toLowerCase()];

            if (defaultValue !== undefined && defaultValue !== null) {
                console.log(`Setting default value for ${fieldName}:`, defaultValue);
                field.value = defaultValue;
                // Only add to flow update using original case
                if (fieldName in defaultValues) {
                    updatedValues[fieldName] = defaultValue;
                }
            }
        });

        // Update flow with original case values only
        if (this.flowContext && Object.keys(updatedValues).length > 0) {
            console.log('Updating flow with applied default values:', updatedValues);
            this.varRecord = { ...this.varRecord, ...updatedValues };
            this.dispatchEvent(
                new FlowAttributeChangeEvent('varRecord', this.varRecord)
            );
        }
    }

    navigateToRecord(event) {
        event.preventDefault();
        const recordId = event.currentTarget.dataset.recordId;
        
        if (recordId) {
            this[NavigationMixin.Navigate]({
                type: 'standard__recordPage',
                attributes: {
                    recordId: recordId,
                    actionName: 'view'
                }
            });
        }
    }
    
    forceRefresh() {
        console.log('Force refresh initiated');
        console.log('wiredFieldValuesResult exists:', !!this.wiredFieldValuesResult);
        this.isLoading = true;
        
        // Add delay to ensure database write is complete
        setTimeout(() => {
            // Use refreshApex to get fresh data from server
            if (this.wiredFieldValuesResult) {
                console.log('Calling refreshApex...');
                refreshApex(this.wiredFieldValuesResult)
                    .then(() => {
                        console.log('RefreshApex completed successfully');
                        // Data will be automatically processed by the wire handler
                    })
                    .catch(error => {
                        console.error('Error during refreshApex:', error);
                        this.error = error;
                        this.isLoading = false;
                    });
            } else {
                console.log('No wiredFieldValuesResult, triggering field update to activate wire');
                // Trigger wire by updating field names
                const tempFields = [...this.fieldApiNamesForWire];
                this.fieldApiNamesForWire = [];
                Promise.resolve().then(() => {
                    this.fieldApiNamesForWire = tempFields;
                });
            }
        }, 500);
    }
    
    forceUIUpdate() {
        // Force the component to re-render by manipulating tracked properties
        const tempSections = [...this.sections];
        this.sections = [];
        
        // Use setTimeout to ensure the DOM registers the change
        setTimeout(() => {
            this.sections = tempSections;
            // Increment key to force template re-evaluation
            this.refreshKey = this.refreshKey + 1;
        }, 0);
    }
}