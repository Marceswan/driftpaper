import { LightningElement, api, wire } from 'lwc';
import { NavigationMixin } from 'lightning/navigation';
import { refreshApex } from '@salesforce/apex';
import { getObjectInfo } from 'lightning/uiObjectInfoApi';
import OPPORTUNITY_OBJECT from '@salesforce/schema/Opportunity';
import getOpportunities from '@salesforce/apex/OpportunityCardsController.getOpportunities';
import OpportunityCardsModal from 'c/opportunityCardsModal';

const LIMIT_SINGLE = 15;
const LIMIT_TABBED_MULTI = 10;

const BASE_SORT_OPTIONS = [
    { value: 'CloseDate-ASC', label: 'Close Date (Earliest First)', field: 'CloseDate', direction: 'ASC' },
    { value: 'CloseDate-DESC', label: 'Close Date (Latest First)', field: 'CloseDate', direction: 'DESC' },
    { value: 'Amount-DESC', label: 'Amount (Highest First)', field: 'Amount', direction: 'DESC' },
    { value: 'Amount-ASC', label: 'Amount (Lowest First)', field: 'Amount', direction: 'ASC' },
    { value: 'Name-ASC', label: 'Name (A-Z)', field: 'Name', direction: 'ASC' },
    { value: 'Name-DESC', label: 'Name (Z-A)', field: 'Name', direction: 'DESC' },
    { value: 'StageName-ASC', label: 'Stage (A-Z)', field: 'StageName', direction: 'ASC' },
    { value: 'LastModifiedDate-DESC', label: 'Last Modified (Recent First)', field: 'LastModifiedDate', direction: 'DESC' },
    { value: 'CreatedDate-DESC', label: 'Created Date (Recent First)', field: 'CreatedDate', direction: 'DESC' }
];

export default class OpportunityCards extends NavigationMixin(LightningElement) {
    @api recordId;
    @api displayMode = 'single';
    @api highlightFields = 'Amount,Probability';

    isLoading = true;
    error;
    wiredResult;
    fieldLabels = {};

    // Data
    openOpportunities = [];
    closedOpportunities = [];
    totalOpenCount = 0;
    totalClosedCount = 0;

    // Sort state
    currentSort = 'CloseDate-ASC';
    openSort = 'CloseDate-ASC';
    closedSort = 'CloseDate-ASC';

    // Mode checks
    get isSingleMode() { return this.displayMode === 'single'; }
    get isTabbedMode() { return this.displayMode === 'tabbed'; }
    get isMultiMode() { return this.displayMode === 'multi'; }

    get displayLimit() {
        return this.isSingleMode ? LIMIT_SINGLE : LIMIT_TABBED_MULTI;
    }

    get fieldList() {
        return this.highlightFields ? this.highlightFields.split(',').map(f => f.trim()) : [];
    }

    // Wire object info for field labels
    @wire(getObjectInfo, { objectApiName: OPPORTUNITY_OBJECT })
    wiredObjectInfo({ error, data }) {
        if (data) {
            this.fieldLabels = {};
            Object.keys(data.fields).forEach(fieldName => {
                this.fieldLabels[fieldName] = data.fields[fieldName].label;
            });
        }
    }

    // Wire data
    @wire(getOpportunities, {
        accountId: '$recordId',
        fields: '$fieldList',
        sortField: '$currentSortField',
        sortDirection: '$currentSortDirection',
        limitCount: '$wireLimitCount'
    })
    wiredOpportunities(result) {
        this.wiredResult = result;
        this.isLoading = false;

        if (result.error) {
            this.error = result.error;
            this.openOpportunities = [];
            this.closedOpportunities = [];
        } else if (result.data) {
            this.error = undefined;
            this.openOpportunities = result.data.openOpportunities || [];
            this.closedOpportunities = result.data.closedOpportunities || [];
            this.totalOpenCount = result.data.totalOpenCount || 0;
            this.totalClosedCount = result.data.totalClosedCount || 0;
        }
    }

    get wireLimitCount() {
        // For initial load, get enough for both sections
        return Math.max(LIMIT_SINGLE, LIMIT_TABBED_MULTI);
    }

    get currentSortField() {
        const option = BASE_SORT_OPTIONS.find(o => o.value === this.currentSort);
        return option ? option.field : 'CloseDate';
    }

    get currentSortDirection() {
        const option = BASE_SORT_OPTIONS.find(o => o.value === this.currentSort);
        return option ? option.direction : 'ASC';
    }

    get hasError() { return !!this.error; }
    get errorMessage() {
        if (!this.error) return '';
        return this.error.body?.message || this.error.message || 'An error occurred';
    }

    // Opportunity getters
    get allOpportunities() {
        return [...this.openOpportunities, ...this.closedOpportunities].slice(0, LIMIT_SINGLE);
    }

    get displayedOpenOpportunities() {
        return this.openOpportunities.slice(0, LIMIT_TABBED_MULTI);
    }

    get displayedClosedOpportunities() {
        return this.closedOpportunities.slice(0, LIMIT_TABBED_MULTI);
    }

    get hasOpportunities() {
        return this.totalOpenCount > 0 || this.totalClosedCount > 0;
    }

    get hasOpenOpportunities() {
        return this.totalOpenCount > 0;
    }

    get hasClosedOpportunities() {
        return this.totalClosedCount > 0;
    }

    // View All
    get showViewAllSingle() {
        return (this.totalOpenCount + this.totalClosedCount) > LIMIT_SINGLE;
    }

    get showViewAllOpen() {
        return this.totalOpenCount > LIMIT_TABBED_MULTI;
    }

    get showViewAllClosed() {
        return this.totalClosedCount > LIMIT_TABBED_MULTI;
    }

    get viewAllLabel() {
        const total = this.totalOpenCount + this.totalClosedCount;
        return `View All (${total})`;
    }

    get viewAllOpenLabel() {
        return `View All (${this.totalOpenCount})`;
    }

    get viewAllClosedLabel() {
        return `View All (${this.totalClosedCount})`;
    }

    // Tab labels
    get openTabLabel() {
        return `Open (${this.totalOpenCount})`;
    }

    get closedTabLabel() {
        return `Closed (${this.totalClosedCount})`;
    }

    // Sort options
    get sortOptions() {
        return this.buildSortOptions(this.currentSort);
    }

    get openSortOptions() {
        return this.buildSortOptions(this.openSort);
    }

    get closedSortOptions() {
        return this.buildSortOptions(this.closedSort);
    }

    buildSortOptions(currentValue) {
        const options = [...BASE_SORT_OPTIONS];

        // Add highlight field sort options
        this.fieldList.forEach(field => {
            if (!BASE_SORT_OPTIONS.some(o => o.field === field)) {
                options.push({
                    value: `${field}-ASC`,
                    label: `${this.fieldLabels[field] || field} (Asc)`,
                    field: field,
                    direction: 'ASC'
                });
                options.push({
                    value: `${field}-DESC`,
                    label: `${this.fieldLabels[field] || field} (Desc)`,
                    field: field,
                    direction: 'DESC'
                });
            }
        });

        return options.map(opt => ({
            ...opt,
            checked: opt.value === currentValue
        }));
    }

    // Event handlers
    handleSortChange(event) {
        this.currentSort = event.detail.value;
        this.handleRefresh();
    }

    handleOpenSortChange(event) {
        this.openSort = event.detail.value;
        // Re-sort locally for open opportunities
        this.sortOpportunities('open');
    }

    handleClosedSortChange(event) {
        this.closedSort = event.detail.value;
        // Re-sort locally for closed opportunities
        this.sortOpportunities('closed');
    }

    sortOpportunities(type) {
        const sortValue = type === 'open' ? this.openSort : this.closedSort;
        const option = BASE_SORT_OPTIONS.find(o => o.value === sortValue) ||
                       this.buildSortOptions(sortValue).find(o => o.value === sortValue);

        if (!option) return;

        const list = type === 'open' ? [...this.openOpportunities] : [...this.closedOpportunities];

        list.sort((a, b) => {
            let aVal = a[option.field] || a.fields?.[option.field];
            let bVal = b[option.field] || b.fields?.[option.field];

            if (aVal === null || aVal === undefined) aVal = '';
            if (bVal === null || bVal === undefined) bVal = '';

            let comparison = 0;
            if (typeof aVal === 'string') {
                comparison = aVal.localeCompare(bVal);
            } else {
                comparison = aVal < bVal ? -1 : (aVal > bVal ? 1 : 0);
            }

            return option.direction === 'DESC' ? -comparison : comparison;
        });

        if (type === 'open') {
            this.openOpportunities = list;
        } else {
            this.closedOpportunities = list;
        }
    }

    handleRefresh() {
        this.isLoading = true;
        refreshApex(this.wiredResult);
    }

    handleNew() {
        this[NavigationMixin.Navigate]({
            type: 'standard__objectPage',
            attributes: {
                objectApiName: 'Opportunity',
                actionName: 'new'
            },
            state: {
                defaultFieldValues: `AccountId=${this.recordId}`
            }
        });
    }

    // View All handlers
    handleViewAll() {
        this.openModal('all', [...this.openOpportunities, ...this.closedOpportunities]);
    }

    handleViewAllOpen() {
        // Tabbed mode navigates to related list
        this[NavigationMixin.Navigate]({
            type: 'standard__recordRelationshipPage',
            attributes: {
                recordId: this.recordId,
                objectApiName: 'Account',
                relationshipApiName: 'Opportunities',
                actionName: 'view'
            }
        });
    }

    handleViewAllClosed() {
        // Tabbed mode navigates to related list
        this[NavigationMixin.Navigate]({
            type: 'standard__recordRelationshipPage',
            attributes: {
                recordId: this.recordId,
                objectApiName: 'Account',
                relationshipApiName: 'Opportunities',
                actionName: 'view'
            }
        });
    }

    handleViewAllOpenModal() {
        this.openModal('open', this.openOpportunities);
    }

    handleViewAllClosedModal() {
        this.openModal('closed', this.closedOpportunities);
    }

    async openModal(type, opportunities) {
        await OpportunityCardsModal.open({
            size: 'large',
            label: type === 'all' ? 'All Opportunities' :
                   type === 'open' ? 'Open Opportunities' : 'Closed Opportunities',
            opportunities: opportunities,
            highlightFields: this.highlightFields,
            fieldLabels: this.fieldLabels,
            totalCount: type === 'all' ? (this.totalOpenCount + this.totalClosedCount) :
                        type === 'open' ? this.totalOpenCount : this.totalClosedCount
        });
    }
}
