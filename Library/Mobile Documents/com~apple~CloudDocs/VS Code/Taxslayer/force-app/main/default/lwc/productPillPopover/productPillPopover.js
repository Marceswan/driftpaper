import { LightningElement, api } from 'lwc';

const MAX_NAME_LENGTH = 20;
const HOVER_DELAY_MS = 200;

export default class ProductPillPopover extends LightningElement {
    @api productName;
    @api quantity;
    @api unitPrice;
    @api totalPrice;
    @api description;

    showPopover = false;
    hoverTimeout;

    get displayName() {
        if (!this.productName) return '';
        if (this.productName.length <= MAX_NAME_LENGTH) {
            return this.productName;
        }
        return this.productName.substring(0, MAX_NAME_LENGTH - 1) + '…';
    }

    handleMouseEnter() {
        this.hoverTimeout = setTimeout(() => {
            this.showPopover = true;
        }, HOVER_DELAY_MS);
    }

    handleMouseLeave() {
        if (this.hoverTimeout) {
            clearTimeout(this.hoverTimeout);
        }
        this.showPopover = false;
    }

    disconnectedCallback() {
        if (this.hoverTimeout) {
            clearTimeout(this.hoverTimeout);
        }
    }
}
