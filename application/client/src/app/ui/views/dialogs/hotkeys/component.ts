import { Component, ChangeDetectorRef, AfterViewChecked, AfterViewInit } from '@angular/core';
import { Ilc, IlcInterface } from '@env/decorators/component';
import { KeysMap, KeyDescription } from '@service/hotkeys/map';
import { ChangesDetector } from '@ui/env/extentions/changes';
import { Initial } from '@env/decorators/initial';

interface IKey {
    shortkeys: string[];
    description: string;
}

interface IGroup {
    name: string;
    keys: IKey[];
}

@Component({
    selector: 'app-dialogs-hotkeys',
    templateUrl: './template.html',
    styleUrls: ['./styles.less'],
})
@Initial()
@Ilc()
export class Hotkeys extends ChangesDetector implements AfterViewChecked, AfterViewInit {
    public groups: IGroup[] = [];
    constructor(cdRef: ChangeDetectorRef) {
        super(cdRef);
        const groups: any = {};
        KeysMap.forEach((desc: KeyDescription) => {
            if (groups[desc.category] === undefined) {
                groups[desc.category] = {
                    name: desc.category,
                    keys: [],
                };
            }
            groups[desc.category].keys.push({
                shortkeys: desc.shortkeys,
                description: desc.description,
            });
        });
        Object.keys(groups).forEach((key: string) => {
            this.groups.push(groups[key]);
        });
    }

    ngAfterViewChecked(): void {
        this.detectChanges();
    }

    ngAfterViewInit(): void {
        this.detectChanges();
    }
}
export interface Hotkeys extends IlcInterface {}
