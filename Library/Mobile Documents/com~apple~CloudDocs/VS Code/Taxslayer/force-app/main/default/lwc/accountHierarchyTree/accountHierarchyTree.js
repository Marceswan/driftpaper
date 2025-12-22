import { LightningElement, api, wire } from 'lwc';
import { NavigationMixin } from 'lightning/navigation';
import { getObjectInfo } from 'lightning/uiObjectInfoApi';
import ACCOUNT_OBJECT from '@salesforce/schema/Account';
import getAccountHierarchy from '@salesforce/apex/AccountHierarchyController.getAccountHierarchy';

const DEFAULT_COLUMNS = 'Name,Industry,AnnualRevenue';

export default class AccountHierarchyTree extends NavigationMixin(LightningElement) {
    @api recordId;
    @api columns = DEFAULT_COLUMNS;

    treeData = [];
    expandedRowIds = [];
    currentAccountId;
    error;
    isLoading = true;
    fieldLabels = {};
    isAllExpanded = true;
    allRowIds = [];
    pathToCurrentAccount = [];

    get gridColumns() {
        return this.parseColumns();
    }

    get hasData() {
        return this.treeData && this.treeData.length > 0;
    }

    get errorMessage() {
        if (!this.error) return '';
        return this.error.body?.message || this.error.message || 'An error occurred';
    }

    get expandCollapseIcon() {
        return this.isAllExpanded ? 'utility:collapse_all' : 'utility:expand_all';
    }

    get expandCollapseLabel() {
        return this.isAllExpanded ? 'Collapse All' : 'Expand All';
    }

    get fieldNames() {
        if (!this.columns) return ['Name'];

        // If JSON array, extract fieldNames
        try {
            const parsed = JSON.parse(this.columns);
            if (Array.isArray(parsed)) {
                return parsed.map(col => col.fieldName || col.field).filter(Boolean);
            }
        } catch (e) {
            // Not JSON, treat as comma-separated
        }

        return this.columns.split(',').map(f => f.trim()).filter(Boolean);
    }

    @wire(getObjectInfo, { objectApiName: ACCOUNT_OBJECT })
    wiredObjectInfo({ error, data }) {
        if (data) {
            // Build map of field API name to label
            const fields = data.fields;
            this.fieldLabels = {};
            Object.keys(fields).forEach(fieldName => {
                this.fieldLabels[fieldName] = fields[fieldName].label;
            });
        } else if (error) {
            console.error('Error loading Account object info:', error);
        }
    }

    @wire(getAccountHierarchy, { recordId: '$recordId', fields: '$fieldNames' })
    wiredHierarchy({ error, data }) {
        this.isLoading = false;

        if (error) {
            this.error = error;
            this.treeData = [];
            return;
        }

        if (data) {
            this.error = undefined;
            this.currentAccountId = data.currentAccountId;
            this.treeData = this.transformToTreeData(data.nodes);
            this.allRowIds = this.collectAllIds(this.treeData);
            this.pathToCurrentAccount = this.findPathToCurrentAccount(this.treeData, this.currentAccountId);
            this.expandedRowIds = [...this.allRowIds];
            this.isAllExpanded = true;
        }
    }

    handleExpandCollapseToggle() {
        if (this.isAllExpanded) {
            // Collapse all except the path to current account
            this.expandedRowIds = [...this.pathToCurrentAccount];
            this.isAllExpanded = false;
        } else {
            // Expand all
            this.expandedRowIds = [...this.allRowIds];
            this.isAllExpanded = true;
        }
    }

    findPathToCurrentAccount(nodes, targetId) {
        if (!nodes || !targetId) return [];

        // Recursive helper that returns null if not found, or array of ancestor IDs if found
        const findPath = (nodeList, ancestorPath) => {
            for (const node of nodeList) {
                if (node.id === targetId) {
                    // Found! Return the ancestor path (not including target itself)
                    return ancestorPath;
                }
                if (node._children) {
                    // Search children, adding current node to ancestor path
                    const result = findPath(node._children, [...ancestorPath, node.id]);
                    if (result !== null) {
                        return result;
                    }
                }
            }
            return null; // Not found in this branch
        };

        return findPath(nodes, []) || [];
    }

    parseColumns() {
        if (!this.columns) {
            return this.buildDefaultColumns();
        }

        // Try JSON parse first
        try {
            const parsed = JSON.parse(this.columns);
            if (Array.isArray(parsed)) {
                return this.ensureNameColumnIsUrl(parsed);
            }
        } catch (e) {
            // Not JSON, treat as comma-separated
        }

        // Comma-separated string
        const fields = this.columns.split(',').map(f => f.trim()).filter(Boolean);
        return this.buildColumnsFromFields(fields);
    }

    buildDefaultColumns() {
        return this.buildColumnsFromFields(['Name', 'Industry', 'AnnualRevenue']);
    }

    buildColumnsFromFields(fields) {
        return fields.map((field, index) => {
            // First column (Name) should be URL type for navigation
            if (field === 'Name' || index === 0) {
                return {
                    type: 'url',
                    fieldName: 'recordUrl',
                    label: this.getFieldLabel(field),
                    typeAttributes: {
                        label: { fieldName: field },
                        target: '_self'
                    },
                    cellAttributes: {
                        class: { fieldName: 'rowClass' }
                    }
                };
            }

            return {
                type: 'text',
                fieldName: field,
                label: this.getFieldLabel(field),
                cellAttributes: {
                    class: { fieldName: 'rowClass' }
                }
            };
        });
    }

    ensureNameColumnIsUrl(columns) {
        return columns.map((col, index) => {
            const fieldName = col.fieldName || col.field;

            // Make Name column a URL for navigation
            if (fieldName === 'Name' || index === 0) {
                return {
                    ...col,
                    type: 'url',
                    fieldName: 'recordUrl',
                    typeAttributes: {
                        label: { fieldName: fieldName || 'Name' },
                        target: '_self'
                    },
                    cellAttributes: {
                        ...col.cellAttributes,
                        class: { fieldName: 'rowClass' }
                    }
                };
            }

            return {
                ...col,
                cellAttributes: {
                    ...col.cellAttributes,
                    class: { fieldName: 'rowClass' }
                }
            };
        });
    }

    getFieldLabel(fieldName) {
        if (!fieldName) return '';

        // Use actual field label from schema if available
        if (this.fieldLabels[fieldName]) {
            return this.fieldLabels[fieldName];
        }

        // Fallback: Convert camelCase or PascalCase to Title Case with spaces
        return fieldName
            .replace(/([A-Z])/g, ' $1')
            .replace(/^./, str => str.toUpperCase())
            .trim();
    }

    transformToTreeData(nodes) {
        if (!nodes) return [];

        return nodes.map(node => {
            const record = {
                id: node.id,
                recordUrl: '/' + node.id,
                rowClass: node.id === this.currentAccountId ? 'slds-theme_success' : ''
            };

            // Only add _children if there are actual children (prevents chevron on leaf nodes)
            const hasChildren = node.children && Array.isArray(node.children) && node.children.length > 0;
            if (hasChildren) {
                record._children = this.transformToTreeData(node.children);
            }

            // Flatten fields onto record
            if (node.fields) {
                Object.keys(node.fields).forEach(key => {
                    record[key] = node.fields[key];
                });
            }

            return record;
        });
    }

    collectAllIds(nodes) {
        const ids = [];

        const traverse = (nodeList) => {
            if (!nodeList) return;
            nodeList.forEach(node => {
                ids.push(node.id);
                if (node._children) {
                    traverse(node._children);
                }
            });
        };

        traverse(nodes);
        return ids;
    }
}
