import { LightningElement, api } from 'lwc';

export default class Stencil extends LightningElement {
    @api iterations = 4;
    @api columns = 2;
    
    get stencilRows() {
        const rows = [];
        for (let i = 0; i < this.iterations; i++) {
            rows.push({
                key: i,
                columns: this.getColumns()
            });
        }
        return rows;
    }
    
    getColumns() {
        const cols = [];
        for (let i = 0; i < this.columns; i++) {
            cols.push({
                key: i,
                class: `slds-col slds-size_1-of-${this.columns}`
            });
        }
        return cols;
    }
}