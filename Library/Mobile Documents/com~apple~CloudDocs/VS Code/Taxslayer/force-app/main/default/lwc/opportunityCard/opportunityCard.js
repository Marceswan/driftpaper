import { LightningElement, api } from 'lwc';
import { NavigationMixin } from 'lightning/navigation';

const MAX_DISPLAYED_PRODUCTS = 10;
const CURRENCY_FIELDS = ['Amount', 'ExpectedRevenue'];
const PERCENT_FIELDS = ['Probability'];
const DATE_FIELDS = ['CloseDate', 'CreatedDate', 'LastModifiedDate'];

export default class OpportunityCard extends NavigationMixin(LightningElement) {
    @api opportunity;
    @api highlightFields = '';
    @api fieldLabels = {};

    get opportunityUrl() {
        return '/' + this.opportunity?.id;
    }

    get formattedCloseDate() {
        if (!this.opportunity?.closeDate) return '';
        const date = new Date(this.opportunity.closeDate);
        return date.toLocaleDateString();
    }

    get isPastDue() {
        if (!this.opportunity?.closeDate) return false;
        const closeDate = new Date(this.opportunity.closeDate);
        const today = new Date();
        today.setHours(0, 0, 0, 0);
        return closeDate < today;
    }

    get stageBadgeClass() {
        let baseClass = 'slds-badge slds-m-left_x-small';
        if (!this.opportunity?.isClosed) {
            return baseClass + ' slds-badge_inverse';
        }
        if (this.opportunity.isWon) {
            return baseClass + ' slds-theme_success';
        }
        return baseClass + ' slds-theme_warning';
    }

    get closeDateBadgeClass() {
        let baseClass = 'slds-badge slds-m-left_x-small';
        if (this.isPastDue) {
            return baseClass + ' slds-theme_error';
        }
        return baseClass + ' slds-badge_inverse';
    }

    get highlightFieldsList() {
        if (!this.highlightFields || !this.opportunity) return [];

        const fields = this.highlightFields.split(',').map(f => f.trim()).filter(Boolean);
        return fields.map(apiName => {
            const value = this.opportunity.fields?.[apiName];
            const label = this.fieldLabels[apiName] || this.formatFieldLabel(apiName);

            return {
                apiName,
                label,
                value,
                isCurrency: CURRENCY_FIELDS.includes(apiName),
                isPercent: PERCENT_FIELDS.includes(apiName),
                isDate: DATE_FIELDS.includes(apiName),
                isText: !CURRENCY_FIELDS.includes(apiName) &&
                        !PERCENT_FIELDS.includes(apiName) &&
                        !DATE_FIELDS.includes(apiName),
                percentValue: PERCENT_FIELDS.includes(apiName) ? (value / 100) : null
            };
        });
    }

    get hasProducts() {
        return this.opportunity?.lineItems?.length > 0;
    }

    get displayedLineItems() {
        if (!this.opportunity?.lineItems) return [];
        return this.opportunity.lineItems.slice(0, MAX_DISPLAYED_PRODUCTS);
    }

    get hasMoreProducts() {
        return this.opportunity?.lineItems?.length > MAX_DISPLAYED_PRODUCTS;
    }

    get remainingProductCount() {
        if (!this.opportunity?.lineItems) return 0;
        return this.opportunity.lineItems.length - MAX_DISPLAYED_PRODUCTS;
    }

    get moreProductsTooltip() {
        return `${this.remainingProductCount} more products`;
    }

    formatFieldLabel(apiName) {
        return apiName
            .replace(/([A-Z])/g, ' $1')
            .replace(/^./, str => str.toUpperCase())
            .trim();
    }

    handleNavigate(event) {
        event.preventDefault();
        this[NavigationMixin.Navigate]({
            type: 'standard__recordPage',
            attributes: {
                recordId: this.opportunity.id,
                actionName: 'view'
            }
        });
    }

    handleEdit(event) {
        event.stopPropagation();
        this[NavigationMixin.Navigate]({
            type: 'standard__recordPage',
            attributes: {
                recordId: this.opportunity.id,
                actionName: 'edit'
            }
        });
    }

    handleShowMoreProducts() {
        // Dispatch event for parent to handle (could open modal with all products)
        this.dispatchEvent(new CustomEvent('showmoreproducts', {
            detail: {
                opportunityId: this.opportunity.id,
                lineItems: this.opportunity.lineItems
            }
        }));
    }
}
