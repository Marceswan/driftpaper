import { api } from 'lwc';
import LightningModal from 'lightning/modal';

export default class OpportunityCardsModal extends LightningModal {
    @api opportunities = [];
    @api highlightFields = '';
    @api fieldLabels = {};
    @api totalCount = 0;
    @api label = 'Opportunities';

    get modalLabel() {
        return `${this.label} (${this.totalCount})`;
    }

    handleClose() {
        this.close('closed');
    }
}
