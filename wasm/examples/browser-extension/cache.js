import fs from 'fs';

export class CacheManager{
    constructor(config={file:"cache.json"}){
        this.config = config
        this.data = {};
        this.load();
    }

    load(){
        fs.stat(this.config.file, (err)=>{
            if (!err){
                this.data = JSON.parse(fs.readFileSync(this.config.file)+"");
            }
        });
    }

    save(){
        fs.writeFileSync(this.config.file, JSON.stringify(this.data, null, "\t"));
    }

    /**
     * @param {string} key
     * @param {any} defaultValue
     * @returns {any}
     */
    get(key, defaultValue){
        return this.data[key]??defaultValue
    }

    /**
     * @param {string} key
     * @param {any} value
     */
    set(key, value){
        this.data[key] = value;
        this.save()
    }


    /**
     * @param {string} key
     */
    delete(key){
        if (this.data[key] != undefined){
            delete this.data[key];
            this.save()
        }
    }

}