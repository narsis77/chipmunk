import { Component, AfterViewInit, ChangeDetectorRef } from '@angular/core';
import { TabsService } from '@elements/tabs/service';
import { LayoutWorkspaceNoContent } from './no-tabs-content/component';
import { Ilc, IlcInterface } from '@env/decorators/component';
import { ChangesDetector } from '@ui/env/extentions/changes';
import { components } from '@env/decorators/initial';

@Component({
    selector: 'app-layout-workspace',
    templateUrl: './template.html',
    styleUrls: ['./styles.less'],
})
@Ilc()
export class LayoutWorkspace extends ChangesDetector implements AfterViewInit {
    public tabs: TabsService;
    constructor(cdRef: ChangeDetectorRef) {
        super(cdRef);
        this.tabs = this.ilc().services.system.session.getTabsService();
    }

    ngAfterViewInit() {
        this.ilc()
            .services.system.session.add()
            .unbound({
                uuid: 'welcome',
                sidebar: true,
                toolbar: false,
                tab: {
                    content: {
                        factory: LayoutWorkspaceNoContent,
                    },
                    active: true,
                    name: '',
                    closable: false,
                    icon: 'home',
                },
            })
            .sidebar()
            ?.add({
                content: {
                    factory: components.get('app-elements-tree'),
                },
                active: true,
                closable: false,
                name: 'Favourite',
            });
    }
}
export interface LayoutWorkspace extends IlcInterface {}
