class RangeDict {
    constructor() {
        this.dict = []; // {start_min, start, end, value}
    }

    // Find the first entry with an end higher than v
    find_end(v) {
        let low = -1;
        let high = this.dict.length;
        while (high > low + 1) {
            let mid = Math.floor((low + high) / 2);
            if (this.dict[mid].end <= v) {
                low = mid;
            } else {
                high = mid;
            }

        }
        return high;
    }

    insert(start, end, value) {
        let pos = this.find_end(end);
        let start_min = start;
        if (pos < this.dict.length && this.dict[pos].start_min < start_min) {
            start_min = this.dict[pos].start_min;
        }
        this.dict.splice(pos, 0, { start_min: start_min, start: start, end: end, value: value });
        while (pos > 0) {
            pos--;
            if (this.dict[pos].start_min <= start_min) break;
            this.dict[pos].start_min = start_min;
        }
    }

    overlapping(start, end) {
        let pos = this.find_end(start);
        let res = [];
        while (pos < this.dict.length) {
            let entry = this.dict[pos];
            if (entry.start_min >= end) break;
            if (entry.end > start && entry.start < end) {
                res.push({ start: entry.start, end: entry.end, value: entry.value });
            }
            pos++;
        }
        return res;
    }
}