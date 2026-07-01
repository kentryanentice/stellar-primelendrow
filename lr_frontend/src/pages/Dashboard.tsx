import { useState } from 'react'
import Sidebar from '../elements/Sidebar'

function Dashboard() {
    const [collapsed, setCollapsed] = useState(false)

    return (
        <div className='dashboard'>
            <Sidebar collapsed={collapsed} onToggleCollapsed={() => setCollapsed(v => !v)} />
            <main className={`dashboard-content${collapsed ? ' is-collapsed' : ''}`}>Dashboard</main>
        </div>
    )
}

export default Dashboard
