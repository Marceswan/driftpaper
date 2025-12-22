export function parseFlexiPageJson(flexiPageJson, recordData) {
    const sections = {}; // Object to store sections and their fields

    console.log('Starting to parse flexiPageJson:', flexiPageJson);

    // First pass: Collect sections and their labels
    flexiPageJson.flexiPageRegions.forEach(region => {
        region.itemInstances.forEach(itemInstance => {
            if (itemInstance.componentInstance && itemInstance.componentInstance.componentName === 'flexipage:fieldSection') {
                const sectionFacetId = itemInstance.componentInstance.componentInstanceProperties.find(prop => prop.name === 'columns').value;
                const sectionLabel = itemInstance.componentInstance.componentInstanceProperties.find(prop => prop.name === 'label').value || 'Unnamed Section';

                if (!sections[sectionFacetId]) {
                    sections[sectionFacetId] = { label: sectionLabel, columns: {} };
                }
            }
        });
    });

    // Second pass: Collect columns and assign to sections
    flexiPageJson.flexiPageRegions.forEach(region => {
        if (region.type === 'Facet') {
            region.itemInstances.forEach(itemInstance => {
                if (itemInstance.componentInstance && itemInstance.componentInstance.componentName === 'flexipage:column') {
                    const columnFacetId = itemInstance.componentInstance.componentInstanceProperties.find(prop => prop.name === 'body').value;
                    const sectionFacetId = region.name;
                    const side = parseInt(itemInstance.componentInstance.identifier.replace('flexipage_column', ''), 10) % 2 === 0 ? 'right' : 'left';

                    if (sections[sectionFacetId]) {
                        sections[sectionFacetId].columns[columnFacetId] = { side: side, fields: {} };
                    }
                }
            });
        }
    });

    // Third pass: Assign fields to columns
    flexiPageJson.flexiPageRegions.forEach(region => {
        if (region.type === 'Facet') {
            region.itemInstances.forEach(itemInstance => {
                if (itemInstance.fieldInstance) {
                    const fieldApiName = itemInstance.fieldInstance.fieldItem.replace('Record.', '');
                    const columnFacetId = region.name;
                    const sectionFacetId = Object.keys(sections).find(sectionId =>
                        Object.keys(sections[sectionId].columns).includes(columnFacetId)
                    );

                    if (sectionFacetId && sections[sectionFacetId].columns[columnFacetId]) {
                        const fieldValue = itemInstance.fieldInstance?.fieldInstanceProperties?.find(prop => prop.name === 'value')?.value || '';
                        const isVisible = true; // Initially set all fields to not visible
                        const isRequired = itemInstance.fieldInstance?.fieldInstanceProperties?.find(prop => prop.name === 'uiBehavior')?.value === 'required';
                        const visibilityRule = itemInstance.fieldInstance?.visibilityRule;

                        console.log(`Field: ${fieldApiName}, Value: ${fieldValue}, isVisible: ${isVisible}, isRequired: ${isRequired}, visibilityRule: ${JSON.stringify(visibilityRule)}`);

                        sections[sectionFacetId].columns[columnFacetId].fields[fieldApiName] = {
                            value: fieldValue,
                            isVisible: isVisible,
                            isRequired: isRequired,
                            visibilityRule: visibilityRule // Add visibility rule to the field
                        };
                    }
                }
            });
        }
    });

    // Remove sections with no fields
    Object.keys(sections).forEach(sectionKey => {
        const section = sections[sectionKey];
        const hasFields = Object.values(section.columns).some(column => Object.keys(column.fields).length > 0);
        if (!hasFields) {
            delete sections[sectionKey];
        }
    });

    console.log('Parsed Sections:', JSON.stringify(sections, null, 2));

    return sections;
}